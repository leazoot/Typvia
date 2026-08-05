//! Tauri command layer: parameter parsing, state locking, clock injection
//! and error mapping only — all behaviour lives in `service` / core.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use tauri::State;

use crate::dto::{
    FolderCreateInput, FolderDto, FolderUpdateInput, LibraryCountsDto, SearchHitDto,
    SnippetCreateInput, SnippetDto, SnippetUpdateInput, TagDto,
};
use crate::error::IpcError;
use crate::service;

/// Single-writer SQLite connection (WAL, database rules): one mutex, no pool.
pub struct AppState {
    conn: Mutex<Connection>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, IpcError> {
        // A poisoned mutex means a command panicked mid-write; surface it as
        // a system error instead of poisoning every later call with a panic.
        self.conn.lock().map_err(|_| IpcError::system())
    }
}

fn now_ms() -> Result<i64, IpcError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| IpcError::system())?;
    i64::try_from(elapsed.as_millis()).map_err(|_| IpcError::system())
}

#[tauri::command]
pub fn snippet_create(
    state: State<'_, AppState>,
    input: SnippetCreateInput,
) -> Result<SnippetDto, IpcError> {
    service::snippet_create(&*state.lock()?, input, now_ms()?)
}

#[tauri::command]
pub fn snippet_update(
    state: State<'_, AppState>,
    input: SnippetUpdateInput,
) -> Result<SnippetDto, IpcError> {
    service::snippet_update(&*state.lock()?, input, now_ms()?)
}

#[tauri::command]
pub fn snippet_get(state: State<'_, AppState>, id: String) -> Result<SnippetDto, IpcError> {
    service::snippet_get(&*state.lock()?, &id)
}

#[tauri::command]
pub fn snippet_list(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::snippet_list(&*state.lock()?, limit, offset)
}

#[tauri::command]
pub fn snippet_list_by_folder(
    state: State<'_, AppState>,
    folder_id: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::snippet_list_by_folder(&*state.lock()?, folder_id.as_deref(), limit, offset)
}

#[tauri::command]
pub fn snippet_list_page(
    state: State<'_, AppState>,
    view: String,
    folder_id: Option<String>,
    snippet_type: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::snippet_list_page(
        &*state.lock()?,
        &view,
        folder_id.as_deref(),
        snippet_type.as_deref(),
        limit,
        offset,
    )
}

#[tauri::command]
pub fn snippet_count(
    state: State<'_, AppState>,
    view: String,
    folder_id: Option<String>,
    snippet_type: Option<String>,
) -> Result<u32, IpcError> {
    service::snippet_count(
        &*state.lock()?,
        &view,
        folder_id.as_deref(),
        snippet_type.as_deref(),
    )
}

#[tauri::command]
pub fn library_counts(state: State<'_, AppState>) -> Result<LibraryCountsDto, IpcError> {
    service::library_counts(&*state.lock()?)
}

#[tauri::command]
pub fn snippet_trash(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::snippet_trash(&*state.lock()?, &id, now_ms()?)
}

#[tauri::command]
pub fn snippet_restore(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::snippet_restore(&*state.lock()?, &id)
}

#[tauri::command]
pub fn snippet_delete_forever(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::snippet_delete_forever(&*state.lock()?, &id)
}

#[tauri::command]
pub fn trash_list(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SnippetDto>, IpcError> {
    service::trash_list(&*state.lock()?, limit, offset)
}

#[tauri::command]
pub fn trash_purge_expired(state: State<'_, AppState>) -> Result<usize, IpcError> {
    service::trash_purge_expired(&*state.lock()?, now_ms()?)
}

#[tauri::command]
pub fn folder_create(
    state: State<'_, AppState>,
    input: FolderCreateInput,
) -> Result<FolderDto, IpcError> {
    service::folder_create(&*state.lock()?, input, now_ms()?)
}

#[tauri::command]
pub fn folder_update(
    state: State<'_, AppState>,
    input: FolderUpdateInput,
) -> Result<FolderDto, IpcError> {
    service::folder_update(&*state.lock()?, input, now_ms()?)
}

#[tauri::command]
pub fn folder_delete(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::folder_delete(&*state.lock()?, &id)
}

#[tauri::command]
pub fn folder_list_children(
    state: State<'_, AppState>,
    parent_id: Option<String>,
) -> Result<Vec<FolderDto>, IpcError> {
    service::folder_list_children(&*state.lock()?, parent_id.as_deref())
}

#[tauri::command]
pub fn tag_create(state: State<'_, AppState>, name: String) -> Result<TagDto, IpcError> {
    service::tag_create(&*state.lock()?, name, now_ms()?)
}

#[tauri::command]
pub fn tag_list(state: State<'_, AppState>) -> Result<Vec<TagDto>, IpcError> {
    service::tag_list(&*state.lock()?)
}

#[tauri::command]
pub fn tag_rename(state: State<'_, AppState>, id: String, name: String) -> Result<(), IpcError> {
    service::tag_rename(&*state.lock()?, &id, &name)
}

#[tauri::command]
pub fn tag_delete(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    service::tag_delete(&*state.lock()?, &id)
}

#[tauri::command]
pub fn search_snippets(
    state: State<'_, AppState>,
    query: String,
    limit: u32,
    offset: u32,
) -> Result<Vec<SearchHitDto>, IpcError> {
    service::search_snippets(&*state.lock()?, &query, limit, offset)
}
