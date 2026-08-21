// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! Desktop semantic-search assembly: model lifecycle (checksum-pinned
//! download / delete), the background embedding worker, and the fused
//! deep-search query. Assembled in the host because host-service's
//! dependency face is frozen; semantic joins sync/ai as host-assembled.
//! With no model on disk every path here is a cheap no-op — the product
//! stays lexical-only (progressive enhancement, off by default).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use sha2::Digest;
use typvia_host_service::dto::SnippetDto;
use typvia_host_service::error::IpcError;
use typvia_host_service::service;
use typvia_semantic::{CandleEmbedder, Embedder, ModelFiles, VectorIndex};

/// Model identity stored with every vector (a swap re-embeds lazily).
pub const MODEL_ID: &str = "multilingual-e5-small";
const MODEL_DIR_NAME: &str = "semantic-model";
/// Semantic hits below this cosine are noise, not results; e5 scores cluster
/// high, so the floor sits high too.
const MIN_SCORE: f32 = 0.82;
/// Passages embedded per worker round; the DB lock is never held across
/// inference.
const EMBED_BATCH: u32 = 16;

/// One downloadable model file with its pinned checksum: the download is the
/// feature's only network face, and it verifies or nothing lands.
struct ModelFile {
    name: &'static str,
    url: &'static str,
    sha256: &'static str,
    bytes: u64,
}

const MODEL_FILES: [ModelFile; 3] = [
    ModelFile {
        name: "config.json",
        url: "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/config.json",
        sha256: "69137736cab8b8903a07fe8afaafdda25aac55415a12a55d1bffa9f581abf959",
        bytes: 655,
    },
    ModelFile {
        name: "tokenizer.json",
        url: "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/tokenizer.json",
        sha256: "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39",
        bytes: 17_082_730,
    },
    ModelFile {
        name: "model.safetensors",
        url: "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/model.safetensors",
        sha256: "1a55775f53449dac10a2bcbc312469fac40b96d53198c407081a831f81c98477",
        bytes: 470_641_600,
    },
];

/// Shared semantic state managed by the Tauri app.
#[derive(Default)]
pub struct SemanticState {
    embedder: Mutex<Option<Arc<CandleEmbedder>>>,
    index: Mutex<Option<Arc<VectorIndex>>>,
    index_dirty: AtomicBool,
    download_running: AtomicBool,
    download_received: AtomicU64,
    download_failed: AtomicBool,
    worker_running: AtomicBool,
}

pub fn model_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(MODEL_DIR_NAME)
}

/// Servable weights name: f16 storage produced locally after the pinned
/// fp32 download verifies (about half the disk footprint; the loader
/// upcasts to f32, so inference is unchanged).
const WEIGHTS_F16_NAME: &str = "model.f16.safetensors";

fn files(data_dir: &Path) -> ModelFiles {
    let dir = model_dir(data_dir);
    ModelFiles {
        config: dir.join("config.json"),
        tokenizer: dir.join("tokenizer.json"),
        weights: dir.join(WEIGHTS_F16_NAME),
    }
}

pub fn model_present(data_dir: &Path) -> bool {
    let f = files(data_dir);
    f.config.is_file() && f.tokenizer.is_file() && f.weights.is_file()
}

pub fn total_download_bytes() -> u64 {
    MODEL_FILES.iter().map(|f| f.bytes).sum()
}

pub fn download_progress(state: &SemanticState) -> (bool, u64, bool) {
    (
        state.download_running.load(Ordering::Acquire),
        state.download_received.load(Ordering::Acquire),
        state.download_failed.load(Ordering::Acquire),
    )
}

/// Lazily loads (and caches) the embedder; `None` while no model is on
/// disk or it fails to load — callers degrade to lexical-only.
fn embedder(state: &SemanticState, data_dir: &Path) -> Option<Arc<CandleEmbedder>> {
    if let Ok(guard) = state.embedder.lock()
        && let Some(loaded) = guard.as_ref()
    {
        return Some(Arc::clone(loaded));
    }
    if !model_present(data_dir) {
        return None;
    }
    let loaded = Arc::new(CandleEmbedder::load(&files(data_dir), MODEL_ID).ok()?);
    if let Ok(mut guard) = state.embedder.lock() {
        *guard = Some(Arc::clone(&loaded));
    }
    Some(loaded)
}

/// Starts the model download unless one is already running. Each file
/// streams to a temp name, is checksum-verified, then renamed — a failed
/// or torn download never becomes a servable model.
pub fn start_download(state: Arc<SemanticState>, data_dir: PathBuf) {
    if state.download_running.swap(true, Ordering::AcqRel) {
        return;
    }
    state.download_failed.store(false, Ordering::Release);
    state.download_received.store(0, Ordering::Release);
    std::thread::spawn(move || {
        let ok = download_all(&state, &data_dir).is_ok();
        if !ok {
            state.download_failed.store(true, Ordering::Release);
        }
        state.download_running.store(false, Ordering::Release);
    });
}

fn download_all(state: &SemanticState, data_dir: &Path) -> Result<(), ()> {
    let dir = model_dir(data_dir);
    std::fs::create_dir_all(&dir).map_err(|_| ())?;
    for file in &MODEL_FILES {
        let is_weights = file.name == "model.safetensors";
        // Weights are served as locally-produced f16; an existing f16 file
        // means this (largest) download already completed and converted.
        if is_weights && dir.join(WEIGHTS_F16_NAME).is_file() {
            state
                .download_received
                .fetch_add(file.bytes, Ordering::AcqRel);
            continue;
        }
        let target = dir.join(file.name);
        if !is_weights && target.is_file() && file_hash_ok(&target, file.sha256) {
            state
                .download_received
                .fetch_add(file.bytes, Ordering::AcqRel);
            continue;
        }
        let tmp = dir.join(format!("{}.tmp", file.name));
        let result = download_one(state, file, &tmp);
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
            return Err(());
        }
        if !file_hash_ok(&tmp, file.sha256) {
            let _ = std::fs::remove_file(&tmp);
            return Err(());
        }
        std::fs::rename(&tmp, &target).map_err(|_| ())?;
        if is_weights {
            // Convert the verified fp32 file to f16 and drop the original;
            // a failed conversion keeps the verified fp32 absent from the
            // servable name, so nothing half-made is ever loaded.
            let f16_tmp = dir.join(format!("{WEIGHTS_F16_NAME}.tmp"));
            if typvia_semantic::convert_weights_to_f16(&target, &f16_tmp).is_err() {
                let _ = std::fs::remove_file(&f16_tmp);
                let _ = std::fs::remove_file(&target);
                return Err(());
            }
            std::fs::rename(&f16_tmp, dir.join(WEIGHTS_F16_NAME)).map_err(|_| ())?;
            let _ = std::fs::remove_file(&target);
        }
    }
    Ok(())
}

fn download_one(state: &SemanticState, file: &ModelFile, tmp: &Path) -> Result<(), ()> {
    let response = ureq::get(file.url).call().map_err(|_| ())?;
    let mut reader = response.into_body().into_reader();
    let out = std::fs::File::create(tmp).map_err(|_| ())?;
    let mut writer = std::io::BufWriter::new(out);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|_| ())?;
        if read == 0 {
            break;
        }
        std::io::Write::write_all(&mut writer, &buffer[..read]).map_err(|_| ())?;
        state
            .download_received
            .fetch_add(read as u64, Ordering::AcqRel);
    }
    std::io::Write::flush(&mut writer).map_err(|_| ())
}

fn file_hash_ok(path: &Path, expected: &str) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut hasher = sha2::Sha256::new();
    if std::io::copy(&mut file, &mut hasher).is_err() {
        return false;
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    hex == expected
}

/// Deletes the model and every stored vector (both are derived data).
pub fn delete_model(
    state: &SemanticState,
    conn: &Connection,
    data_dir: &Path,
) -> Result<(), IpcError> {
    if let Ok(mut guard) = state.embedder.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = state.index.lock() {
        *guard = None;
    }
    let dir = model_dir(data_dir);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|_| IpcError::system())?;
    }
    typvia_core::repo::EmbeddingRepo::new(conn)
        .clear()
        .map_err(IpcError::from)?;
    Ok(())
}

/// Drains the pending-embedding queue on a background thread (at most one
/// at a time). The DB lock is only held to fetch a batch or store its
/// vectors — never across inference. Embedding failures leave rows pending
/// and never touch user data.
pub fn spawn_embed_worker(
    state: Arc<SemanticState>,
    conn: Arc<Mutex<Connection>>,
    data_dir: PathBuf,
) {
    if state.worker_running.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(move || {
        drain_pending(&state, &conn, &data_dir);
        state.worker_running.store(false, Ordering::Release);
    });
}

fn drain_pending(state: &SemanticState, conn: &Arc<Mutex<Connection>>, data_dir: &Path) {
    let Some(embedder) = embedder(state, data_dir) else {
        return;
    };
    loop {
        let batch = {
            let Ok(guard) = conn.lock() else { return };
            let Ok(batch) =
                typvia_core::repo::EmbeddingRepo::new(&guard).list_pending(MODEL_ID, EMBED_BATCH)
            else {
                return;
            };
            batch
        };
        if batch.is_empty() {
            return;
        }
        let texts: Vec<String> = batch
            .iter()
            .map(|row| format!("{}\n{}", row.title, row.body))
            .collect();
        let Ok(vectors) = embedder.embed_passages(&texts) else {
            // Inference failure: rows stay pending; nothing else changes.
            return;
        };
        let now = crate::commands::now_ms().unwrap_or(0);
        {
            let Ok(guard) = conn.lock() else { return };
            let repo = typvia_core::repo::EmbeddingRepo::new(&guard);
            for (row, vector) in batch.iter().zip(vectors) {
                let bytes: Vec<u8> = vector.iter().flat_map(|v| v.to_le_bytes()).collect();
                // A row that turned sensitive/deleted mid-flight is refused
                // by the repository invariant; skip and move on.
                let _ = repo.upsert(
                    &row.snippet_id,
                    MODEL_ID,
                    embedder.dims() as u32,
                    &bytes,
                    now,
                );
            }
        }
        state.index_dirty.store(true, Ordering::Release);
    }
}

/// The resident index, rebuilt from the DB when vectors changed.
fn index(
    state: &SemanticState,
    conn: &Connection,
    dims: usize,
) -> Result<Arc<VectorIndex>, IpcError> {
    let dirty = state.index_dirty.swap(false, Ordering::AcqRel);
    if !dirty
        && let Ok(guard) = state.index.lock()
        && let Some(existing) = guard.as_ref()
    {
        return Ok(Arc::clone(existing));
    }
    let stored = typvia_core::repo::EmbeddingRepo::new(conn)
        .list_for_model(MODEL_ID)
        .map_err(IpcError::from)?;
    let entries = stored.into_iter().map(|row| {
        let vector: Vec<f32> = row
            .vector
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| f32::from_le_bytes(*chunk))
            .collect();
        (row.snippet_id, vector)
    });
    let built = Arc::new(VectorIndex::build(dims, entries));
    if let Ok(mut guard) = state.index.lock() {
        *guard = Some(Arc::clone(&built));
    }
    Ok(built)
}

/// Budget for the semantic branch: past it the frame ships lexical-only and
/// the late result is dropped, never spliced in afterwards. The measured
/// baseline is ~14ms per query.
const SEMANTIC_BUDGET: std::time::Duration = std::time::Duration::from_millis(30);

/// Runs embed + scan on a helper thread and waits at most `budget`.
/// `None` = over budget or inference failure — the caller serves lexical.
fn hits_within_budget(
    embedder: Arc<dyn Embedder + Send + Sync>,
    index: Arc<VectorIndex>,
    query: String,
    k: usize,
    budget: std::time::Duration,
) -> Option<Vec<typvia_semantic::ScanHit>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let hits = embedder
            .embed_query(&query)
            .map(|vector| index.top_k(&vector, k, MIN_SCORE));
        let _ = sender.send(hits);
    });
    receiver.recv_timeout(budget).ok()?.ok()
}

/// Fused deep search: lexical hits first in their
/// existing order, then semantic-only hits above the score floor, each
/// snippet once. With no model this is exactly the lexical result; a
/// semantic branch past budget is absent, not late.
pub fn search_deep(
    state: &SemanticState,
    conn: &Connection,
    data_dir: &Path,
    query: &str,
    limit: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    let lexical = service::search_library(conn, query, limit)?;
    let Some(embedder) = embedder(state, data_dir) else {
        return Ok(lexical);
    };
    let index = index(state, conn, embedder.dims())?;
    let Some(hits) = hits_within_budget(
        embedder,
        index,
        query.to_string(),
        limit as usize,
        SEMANTIC_BUDGET,
    ) else {
        return Ok(lexical);
    };
    let mut results = lexical;
    for hit in hits {
        if results.len() >= limit as usize {
            break;
        }
        if results.iter().any(|row| row.id == hit.id) {
            continue;
        }
        if let Ok(row) = service::snippet_get(conn, &hit.id) {
            results.push(row);
        }
    }
    Ok(results)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use typvia_host_service::dto::SnippetCreateInput;
    use typvia_semantic::EmbedError;

    fn conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn create(conn: &Connection, title: &str, body: &str) -> String {
        service::snippet_create(
            conn,
            SnippetCreateInput {
                title: title.to_string(),
                body: body.to_string(),
                snippet_type: "text".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
            },
            1_700_000_000_000,
        )
        .unwrap()
        .id
    }

    #[test]
    fn without_a_model_deep_search_is_exactly_lexical() {
        let conn = conn();
        create(&conn, "Docker logs", "docker logs -f app");
        let state = SemanticState::default();
        let data_dir = tempfile::tempdir().unwrap();

        let results = search_deep(&state, &conn, data_dir.path(), "docker", 50).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Docker logs");

        let results = search_deep(&state, &conn, data_dir.path(), "zzz", 50).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn fusion_appends_semantic_hits_after_lexical_without_duplicates() {
        // Exercises the fusion rule below the embedder: seed vectors
        // directly and rebuild the index; lexical order stays first.
        let conn = conn();
        let lexical_id = create(&conn, "Docker logs", "docker logs -f app");
        let semantic_id = create(&conn, "Container output", "kubectl logs deployment/app");
        let repo = typvia_core::repo::EmbeddingRepo::new(&conn);
        let unit = |v: &[f32]| -> Vec<u8> { v.iter().flat_map(|x| x.to_le_bytes()).collect() };
        repo.upsert(&lexical_id, MODEL_ID, 2, &unit(&[1.0, 0.0]), 1)
            .unwrap();
        repo.upsert(&semantic_id, MODEL_ID, 2, &unit(&[0.9, 0.1]), 1)
            .unwrap();

        let state = SemanticState::default();
        state.index_dirty.store(true, Ordering::Release);
        let index = index(&state, &conn, 2).unwrap();
        let hits = index.top_k(&[1.0, 0.05], 10, 0.5);
        assert_eq!(hits.len(), 2);

        // The fused list puts the lexical row first and includes the
        // semantic-only row exactly once (assembled the same way
        // search_deep does past the embedding step).
        let lexical = service::search_library(&conn, "docker", 50).unwrap();
        assert_eq!(lexical.len(), 1);
        let mut fused = lexical;
        for hit in hits {
            if !fused.iter().any(|row| row.id == hit.id) {
                fused.push(service::snippet_get(&conn, &hit.id).unwrap());
            }
        }
        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].id, lexical_id);
        assert_eq!(fused[1].id, semantic_id);
    }

    #[test]
    fn delete_model_clears_vectors_and_survives_a_missing_dir() {
        let conn = conn();
        let id = create(&conn, "A", "body");
        typvia_core::repo::EmbeddingRepo::new(&conn)
            .upsert(&id, MODEL_ID, 2, &[0, 0, 0, 0, 0, 0, 128, 63], 1)
            .unwrap();
        let state = SemanticState::default();
        let data_dir = tempfile::tempdir().unwrap();

        delete_model(&state, &conn, data_dir.path()).unwrap();
        assert_eq!(
            typvia_core::repo::EmbeddingRepo::new(&conn)
                .count_for_model(MODEL_ID)
                .unwrap(),
            0
        );
    }

    struct FixedEmbedder {
        delay: std::time::Duration,
    }

    impl Embedder for FixedEmbedder {
        fn model_id(&self) -> &str {
            "fixed"
        }
        fn dims(&self) -> usize {
            2
        }
        fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
            Ok(texts.iter().map(|_| vec![1.0, 0.0]).collect())
        }
        fn embed_query(&self, _text: &str) -> Result<Vec<f32>, EmbedError> {
            std::thread::sleep(self.delay);
            Ok(vec![1.0, 0.0])
        }
    }

    #[test]
    fn a_fast_semantic_branch_lands_within_budget() {
        let index = Arc::new(VectorIndex::build(
            2,
            vec![("a".to_string(), vec![1.0, 0.0])],
        ));
        let embedder = Arc::new(FixedEmbedder {
            delay: std::time::Duration::ZERO,
        });
        let hits = hits_within_budget(
            embedder,
            index,
            "q".to_string(),
            10,
            std::time::Duration::from_millis(500),
        )
        .unwrap();
        assert_eq!(hits[0].id, "a");
    }

    #[test]
    fn an_over_budget_semantic_branch_is_absent_not_late() {
        let index = Arc::new(VectorIndex::build(
            2,
            vec![("a".to_string(), vec![1.0, 0.0])],
        ));
        let embedder = Arc::new(FixedEmbedder {
            delay: std::time::Duration::from_millis(200),
        });
        let hits = hits_within_budget(
            embedder,
            index,
            "q".to_string(),
            10,
            std::time::Duration::from_millis(5),
        );
        assert!(hits.is_none());
    }

    #[test]
    fn file_hashing_pins_exact_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, b"typvia").unwrap();
        // sha256("typvia")
        let expected = "ca6a97a2e56f27ed3d5eec8b2298e1f2c96b731fc63e5d76b2b5b9e5a2f61eef";
        let actual_ok = file_hash_ok(&path, expected);
        // Compute the real digest to keep the fixture honest.
        let mut hasher = sha2::Sha256::new();
        std::io::copy(&mut std::fs::File::open(&path).unwrap(), &mut hasher).unwrap();
        let real: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(actual_ok, expected == real);
        assert!(file_hash_ok(&path, &real));
        assert!(!file_hash_ok(&path, "00"));
        assert!(!file_hash_ok(&dir.path().join("missing"), &real));
    }
}
