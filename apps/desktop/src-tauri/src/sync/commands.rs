//! Sync command surface. Each command parses its arguments, takes the host
//! guards in the fixed `sync` → `conn` → `vault` order, calls the engine or a
//! host-service use case, and maps the failure — no business logic here.

use rusqlite::Connection;
use tauri::State;
use typvia_core::model::TimestampMs;
use typvia_core::repo::{SnippetRepo, SyncStateRepo};
use typvia_core::vault::{SecureStore, VaultSession};
use typvia_host_service::dto::{ConflictKeep, ConflictPairDto};
use typvia_host_service::error::IpcError;
use typvia_host_service::service;
use typvia_sync::{NewAccount, PairingCode, RecoveryCode, RecoveryRequest, SyncReport};

use super::dto::{
    PairingClaimDto, PairingSasDto, PairingStartDto, RecoveryCodeDto, SyncDeviceDto, SyncRoundDto,
    SyncStatusDto,
};
use super::{
    SyncHost, SyncState, map_sync, mark_recovery_exported, recovery_exported_at, setup_state,
};
use crate::commands::{AppState, now_ms};

/// Current sync state for the Sync page and the Settings group.
#[tauri::command]
pub fn sync_status(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
) -> Result<SyncStatusDto, IpcError> {
    let host = sync.lock()?;
    let conn = state.lock()?;
    let mut vault = state.lock_vault()?;
    status_of(&conn, &mut vault, &host, sync.data_dir())
}

fn status_of(
    conn: &Connection,
    vault: &mut VaultSession,
    host: &SyncHost,
    data_dir: &std::path::Path,
) -> Result<SyncStatusDto, IpcError> {
    let setup = setup_state(conn)?;
    let conflicts = SnippetRepo::new(conn).list_conflict_copies()?.len();
    Ok(SyncStatusDto {
        available: host.available,
        configured: setup.account_id.is_some(),
        enabled: setup.enabled,
        server_url: setup.server_url,
        account_id: setup.account_id,
        device_id: host.device_id.clone(),
        device_name: host.device_name.clone(),
        key_generation: setup.key_generation,
        pending_backlog: setup.pending_backlog,
        conflict_count: u32::try_from(conflicts).unwrap_or(u32::MAX),
        last_sync_at: host.last_sync_at,
        vault_ready: VaultSession::is_initialized(conn)?,
        vault_unlocked: vault.is_unlocked(),
        recovery_exported_at: recovery_exported_at(data_dir),
        recovery_catchup_pending: setup.recovery_catchup_pending,
        transport_kind: setup.transport_kind.clone(),
    })
}

/// Creates the account on `server_url` with this device as its trust root
/// and starts queueing local changes.
#[tauri::command]
pub fn sync_enable(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    server_url: String,
) -> Result<SyncStatusDto, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let mut vault = state.lock_vault()?;
    let platform = service::current_platform()
        .ok_or_else(|| IpcError::conflict("this platform cannot join an account"))?;
    host.attach_engine(state.secure_store(), &server_url)?;
    let device_name = host.device_name.clone();
    let account = NewAccount {
        device_name: &device_name,
        platform,
        server_url: &server_url,
        master_key: None,
    };
    let engine = host.engine_mut()?;
    vault
        .with_master_key(|master_key| {
            engine.create_account(
                &conn,
                state.secure_store(),
                &NewAccount {
                    master_key,
                    ..account
                },
                now,
            )
        })
        .map_err(map_sync)?;
    let key_id = SyncStateRepo::new(&conn).config_get()?.sync_key_id;
    host.register_sealer(&conn, state.secure_store(), key_id)?;
    // A debounced first round follows so the fresh account converges without
    // waiting for the idle poll.
    host.scheduler().note_local_change();
    status_of(&conn, &mut vault, &host, sync.data_dir())
}

/// Founds an account on the user's own WebDAV endpoint. The endpoint is
/// probed for the conditional-request semantics the protocol depends on and
/// refused honestly when it strips them; credentials go into the platform
/// secure store only.
#[tauri::command]
pub fn sync_enable_webdav(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    base_url: String,
    credentials: Option<String>,
) -> Result<SyncStatusDto, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let mut vault = state.lock_vault()?;
    let platform = service::current_platform()
        .ok_or_else(|| IpcError::conflict("this platform cannot join an account"))?;
    let store = state.secure_store();
    store_webdav_credentials(store, credentials.as_deref())?;
    host.attach_webdav(store, &base_url)?;
    host.webdav()?
        .client()
        .probe_preconditions(&typvia_core::repo::new_id())
        .map_err(super::map_transport)?;
    let device_name = host.device_name.clone();
    {
        let webdav = host.is_webdav();
        debug_assert!(webdav);
        let (engine, dav) = host.engine_and_webdav()?;
        vault
            .with_master_key(|master_key| {
                engine.create_webdav_account(
                    &conn,
                    store,
                    dav,
                    &typvia_sync::NewAccount {
                        device_name: &device_name,
                        platform,
                        server_url: &base_url,
                        master_key,
                    },
                    now,
                )
            })
            .map_err(map_sync)?;
    }
    let key_id = SyncStateRepo::new(&conn).config_get()?.sync_key_id;
    host.register_sealer(&conn, store, key_id)?;
    host.scheduler().note_local_change();
    status_of(&conn, &mut vault, &host, sync.data_dir())
}

/// Starts joining an existing WebDAV account from this (new) device: the
/// account id comes from the store's marker file; the pairing code goes to
/// a trusted device for approval.
#[tauri::command]
pub fn pairing_begin_webdav(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    base_url: String,
    credentials: Option<String>,
) -> Result<PairingStartDto, IpcError> {
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let store = state.secure_store();
    store_webdav_credentials(store, credentials.as_deref())?;
    host.attach_webdav(store, &base_url)?;
    let account = host
        .webdav()?
        .read_account()
        .map_err(super::map_transport)?
        .ok_or_else(|| IpcError::conflict("this account has no vault to recover"))?;
    let account_id = account.account_id;
    let handle = {
        let (engine, _) = host.engine_and_webdav()?;
        engine
            .begin_pairing_webdav(&conn, &base_url, &account_id)
            .map_err(map_sync)?
    };
    let started = PairingStartDto {
        code: handle.code().to_string(),
        session_id: handle.session_id().to_string(),
    };
    host.claimed = None;
    host.pairing = Some(handle);
    Ok(started)
}

/// Recovers a WebDAV account on this device with the recovery code:
/// the directory is re-rooted, prior devices self-refuse, and the history
/// catch-up resumes in the rounds.
#[tauri::command]
pub fn recovery_recover_webdav(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    base_url: String,
    credentials: Option<String>,
    code: String,
    master_password: String,
) -> Result<SyncStatusDto, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let mut vault = state.lock_vault()?;
    let parsed = RecoveryCode::parse(&code).map_err(|e| map_sync(e.into()))?;
    let store = state.secure_store();
    store_webdav_credentials(store, credentials.as_deref())?;
    host.attach_webdav(store, &base_url)?;
    let adopted = {
        let (engine, dav) = host.engine_and_webdav()?;
        engine
            .recover_account_webdav(
                &conn,
                store,
                dav,
                &parsed,
                master_password.as_bytes(),
                &base_url,
                now,
            )
            .map_err(map_sync)?
    };
    host.register_sealer(&conn, store, adopted.sync_key_id)?;
    host.scheduler().note_local_change();
    status_of(&conn, &mut vault, &host, sync.data_dir())
}

/// Validates and persists WebDAV credentials into the secure store; `None`
/// clears the entry (anonymous endpoints exist in tests and labs).
fn store_webdav_credentials(
    store: &dyn SecureStore,
    credentials: Option<&str>,
) -> Result<(), IpcError> {
    match credentials {
        Some(raw) => {
            typvia_sync::WebdavCredentials::parse(raw).map_err(super::map_transport)?;
            store
                .store(super::WEBDAV_CREDENTIALS_ENTRY, raw.as_bytes())
                .map_err(|_| IpcError::system())?;
        }
        None => {
            store
                .remove(super::WEBDAV_CREDENTIALS_ENTRY)
                .map_err(|_| IpcError::system())?;
        }
    }
    Ok(())
}

/// Turns sync off. Everything stays: the queue, the keys and the
/// account binding, so switching it back on resumes rather than re-pairs.
#[tauri::command]
pub fn sync_disable(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
) -> Result<SyncStatusDto, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let mut vault = state.lock_vault()?;
    host.engine_mut()?.disable(&conn, now).map_err(map_sync)?;
    host.detach_sealer();
    status_of(&conn, &mut vault, &host, sync.data_dir())
}

/// Switches sync back on for the already-bound account.
#[tauri::command]
pub fn sync_resume(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
) -> Result<SyncStatusDto, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let mut vault = state.lock_vault()?;
    let repo = SyncStateRepo::new(&conn);
    let mut config = repo.config_get()?;
    let server_url = config
        .server_url
        .clone()
        .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
    config.enabled = true;
    config.updated_at = now;
    repo.config_put(&config)?;
    host.attach_engine(state.secure_store(), &server_url)?;
    host.register_sealer(&conn, state.secure_store(), config.sync_key_id)?;
    // Everything written while sync was off is queued here.
    host.reconcile(&conn, state.secure_store(), config.sync_key_id, now)?;
    // The reconciled backlog pushes within the debounce window.
    host.scheduler().note_local_change();
    status_of(&conn, &mut vault, &host, sync.data_dir())
}

/// Runs one push → pull → merge round now.
#[tauri::command]
pub fn sync_now(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
) -> Result<SyncRoundDto, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let started = host.scheduler().begin_round();
    let result = run_round(&state.conn_handle(), state.secure_store(), &mut host, now);
    // Manual rounds feed the schedule too: success re-arms the idle poll and
    // clears any backoff.
    host.scheduler().note_round(started, result.is_ok());
    result
}

/// One round plus the index maintenance that must follow it: the search
/// index lives above `crates/sync` in the dependency order, so applied
/// records are re-indexed here rather than inside the engine.
///
/// Takes the connection mutex rather than a guard: the engine
/// locks it per local phase and releases it during network I/O, so reads
/// elsewhere in the host keep flowing while the round is on the wire.
pub(crate) fn run_round(
    db: &std::sync::Mutex<Connection>,
    store: &dyn SecureStore,
    host: &mut SyncHost,
    now: TimestampMs,
) -> Result<SyncRoundDto, IpcError> {
    let report = host.run_engine_round(db, store, now)?;
    let conn = db.lock().map_err(|_| IpcError::system())?;
    service::reindex_snippets(&conn, &report.applied_snippet_ids)?;
    host.last_sync_at = Some(now);
    Ok(round_dto(&report, now))
}

fn round_dto(report: &SyncReport, now: TimestampMs) -> SyncRoundDto {
    SyncRoundDto {
        pushed: u32::try_from(report.pushed).unwrap_or(u32::MAX),
        applied: u32::try_from(report.applied).unwrap_or(u32::MAX),
        merged: u32::try_from(report.merged).unwrap_or(u32::MAX),
        conflict_copies: u32::try_from(report.conflict_copy_ids.len()).unwrap_or(u32::MAX),
        parked: u32::try_from(report.parked).unwrap_or(u32::MAX),
        skipped: u32::try_from(report.skipped).unwrap_or(u32::MAX),
        pending_backlog: report.pending_backlog,
        at: now,
    }
}

/// The account's devices as the route drawing shows them.
#[tauri::command]
pub fn sync_devices(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
) -> Result<Vec<SyncDeviceDto>, IpcError> {
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let devices = host
        .engine_mut()?
        .account_devices(&conn)
        .map_err(map_sync)?;
    Ok(devices
        .into_iter()
        .map(|device| SyncDeviceDto {
            device_id: device.device_id,
            name: device.name,
            platform: device.platform,
            created_at: device.created_at,
            revoked_at: device.revoked_at,
            verified: device.verified,
            is_this_device: device.is_this_device,
            is_root: device.is_root,
        })
        .collect())
}

/// Revokes another device and rotates K_sync in the same call —
/// a revoked device holds the old generation, so leaving it in place would
/// make the revocation cosmetic.
#[tauri::command]
pub fn sync_revoke_device(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    device_id: String,
) -> Result<u32, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let vault = state.lock_vault()?;
    if device_id == host.device_id {
        return Err(IpcError::validation("a device cannot revoke itself"));
    }
    let store = state.secure_store();
    let key_id = vault.with_master_key(|master_key| {
        host.revoke_on_backend(&conn, store, &device_id, master_key, now)
    })?;
    host.register_sealer(&conn, store, key_id)?;
    Ok(key_id)
}

/// Rotates K_sync on demand.
#[tauri::command]
pub fn sync_rotate_key(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
) -> Result<u32, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let vault = state.lock_vault()?;
    let store = state.secure_store();
    let key_id = vault
        .with_master_key(|master_key| host.rotate_on_backend(&conn, store, master_key, now))?;
    host.register_sealer(&conn, store, key_id)?;
    Ok(key_id)
}

// ----- pairing: the joining device -----

/// Starts a pairing session and returns the code to display.
#[tauri::command]
pub fn pairing_begin(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    server_url: String,
    account_id: String,
) -> Result<PairingStartDto, IpcError> {
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    host.attach_engine(state.secure_store(), &server_url)?;
    let handle = host
        .engine_mut()?
        .begin_pairing(&conn, &server_url, &account_id)
        .map_err(map_sync)?;
    let started = PairingStartDto {
        code: handle.code().to_string(),
        session_id: handle.session_id().to_string(),
    };
    host.claimed = None;
    host.pairing = Some(handle);
    Ok(started)
}

/// Polls for the trusted device's offer. `None` while it has not confirmed.
/// A ready offer is verified and its SAS returned for the user's check —
/// nothing is installed until `pairing_finalize`.
#[tauri::command]
pub fn pairing_poll(sync: State<'_, SyncState>) -> Result<Option<PairingClaimDto>, IpcError> {
    let mut host = sync.lock()?;
    if let Some(claimed) = &host.claimed {
        return Ok(Some(PairingClaimDto {
            sas: claimed.sas().to_string(),
            root_fingerprint: claimed.root_fingerprint().display_code(),
        }));
    }
    let Some(handle) = host.pairing.take() else {
        return Err(IpcError::conflict("no pairing is in progress"));
    };
    let claimed = host.poll_pairing_on_backend(&handle);
    host.pairing = Some(handle);
    let Some(claimed) = claimed? else {
        return Ok(None);
    };
    let dto = PairingClaimDto {
        sas: claimed.sas().to_string(),
        root_fingerprint: claimed.root_fingerprint().display_code(),
    };
    host.claimed = Some(claimed);
    Ok(Some(dto))
}

/// Completes pairing after the user confirmed the SAS on both screens.
/// `master_password` is required when the offer carries vault material: the
/// master key is re-wrapped under this device's own password.
#[tauri::command]
pub fn pairing_finalize(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    master_password: Option<String>,
) -> Result<SyncStatusDto, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let mut vault = state.lock_vault()?;
    let handle = host
        .pairing
        .take()
        .ok_or_else(|| IpcError::conflict("no pairing is in progress"))?;
    let claimed = host
        .claimed
        .take()
        .ok_or_else(|| IpcError::conflict("confirm the pairing code first"))?;
    let store = state.secure_store();
    let adopted = host.finalize_pairing_on_backend(
        &conn,
        store,
        &handle,
        &claimed,
        master_password.as_deref().map(str::as_bytes),
        now,
    )?;
    host.register_sealer(&conn, store, adopted.sync_key_id)?;
    status_of(&conn, &mut vault, &host, sync.data_dir())
}

/// Abandons an in-flight pairing session on this device.
#[tauri::command]
pub fn pairing_cancel(sync: State<'_, SyncState>) -> Result<(), IpcError> {
    let mut host = sync.lock()?;
    host.pairing = None;
    host.claimed = None;
    Ok(())
}

// ----- pairing: the admitting device -----

/// Reads a pairing code and returns the SAS to compare, without admitting
/// anything yet.
#[tauri::command]
pub fn pairing_sas(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    code: String,
) -> Result<PairingSasDto, IpcError> {
    let host = sync.lock()?;
    let conn = state.lock()?;
    let parsed = PairingCode::decode(&code).map_err(|e| map_sync(e.into()))?;
    let engine = host
        .engine
        .as_ref()
        .ok_or_else(|| IpcError::conflict("sync is not set up on this device"))?;
    let sas = engine.pairing_sas(&conn, &parsed).map_err(map_sync)?;
    Ok(PairingSasDto {
        sas,
        device_name: parsed.device_name,
        platform: parsed.platform.as_str().to_string(),
        vault_ready: VaultSession::is_initialized(&conn)?,
    })
}

/// Admits the device after the user confirmed the SAS on both screens.
/// `allow_vault` decides whether the master key and K_vault travel with the
/// bundle; granting it needs an unlocked vault.
#[tauri::command]
pub fn pairing_approve(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    code: String,
    allow_vault: bool,
) -> Result<(), IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let vault = state.lock_vault()?;
    let parsed = PairingCode::decode(&code).map_err(|e| map_sync(e.into()))?;
    let store = state.secure_store();
    vault.with_master_key(|master_key| {
        host.approve_pairing_on_backend(&conn, store, &parsed, allow_vault, master_key, now)
    })
}

// ----- recovery -----

/// Generates a recovery code, uploads the wrapped blob, and returns the code
/// exactly once. Needs an unlocked vault: the blob carries the master key and
/// the rootproof key derives from it.
#[tauri::command]
pub fn recovery_publish(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
) -> Result<RecoveryCodeDto, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let vault = state.lock_vault()?;
    let setup = setup_state(&conn)?;
    let (Some(account_id), Some(server_url)) = (setup.account_id, setup.server_url) else {
        return Err(IpcError::conflict("sync is not set up on this device"));
    };
    let store = state.secure_store();
    let code = vault.with_master_key(|master_key| match master_key {
        Some(master_key) => host.publish_recovery_on_backend(&conn, store, master_key),
        None => Err(IpcError::permission_denied("unlock the vault first")),
    })?;
    mark_recovery_exported(sync.data_dir(), now);
    Ok(RecoveryCodeDto {
        code: code.display_groups(),
        account_id,
        server_url,
    })
}

/// Recovers an account on this device with a recovery code: the
/// trust root is replaced, every previous device is revoked server-side, and
/// K_sync is rotated. `master_password` is the one this device will use for
/// its own vault afterwards.
#[tauri::command]
pub fn recovery_recover(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    server_url: String,
    account_id: String,
    code: String,
    master_password: String,
) -> Result<SyncStatusDto, IpcError> {
    let now = now_ms()?;
    let mut host = sync.lock()?;
    let conn = state.lock()?;
    let mut vault = state.lock_vault()?;
    let parsed = RecoveryCode::parse(&code).map_err(|e| map_sync(e.into()))?;
    host.attach_engine(state.secure_store(), &server_url)?;
    let store = state.secure_store();
    let adopted = host
        .engine_mut()?
        .recover_account(
            &conn,
            store,
            &RecoveryRequest {
                server_url: &server_url,
                account_id: &account_id,
                code: &parsed,
                master_password: master_password.as_bytes(),
            },
            now,
        )
        .map_err(map_sync)?;
    host.register_sealer(&conn, store, adopted.sync_key_id)?;
    status_of(&conn, &mut vault, &host, sync.data_dir())
}

// ----- conflicts -----

/// The conflict pairs awaiting a decision.
#[tauri::command]
pub fn sync_conflicts(state: State<'_, AppState>) -> Result<Vec<ConflictPairDto>, IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    service::sync_conflict_list(&conn, now)
}

/// Records the user's decision. The losing body is never destroyed — it
/// moves to the recycle bin, or stays as its own snippet for `both`.
#[tauri::command]
pub fn sync_conflict_resolve(
    state: State<'_, AppState>,
    copy_id: String,
    keep: ConflictKeep,
) -> Result<(), IpcError> {
    let now = now_ms()?;
    let conn = state.lock()?;
    let session = state.lock_vault()?;
    service::sync_conflict_resolve(&conn, &session, &copy_id, keep, now)
}
