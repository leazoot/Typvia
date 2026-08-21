// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// SPDX-License-Identifier: MPL-2.0

//! System-scheduled background sync entries. Both platforms funnel into the
//! same round the foreground triggers run — there is no second sync
//! orchestration here, only the plumbing that reaches it from a background
//! wake-up. Failures stay silent on purpose: an unreachable server is the
//! offline state, and the log red line covers these paths like every other.

/// iOS: the BGAppRefreshTask handler (gen/apple BackgroundSync.m) calls
/// back into the Rust side of the running app. The app object is fully
/// assembled by `setup`, so the round reuses the complete host state.
#[cfg(target_os = "ios")]
pub(crate) mod ios {
    use std::sync::OnceLock;

    static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

    /// Parks the handle for the background entry; called once from `setup`.
    pub(crate) fn install(app: &tauri::AppHandle) {
        let _ = APP.set(app.clone());
    }

    /// BGAppRefreshTask entry. Returns false when the app has not finished
    /// assembling (a cold background launch racing `setup`) — the handler
    /// then completes the task unsuccessfully and the system retries later.
    #[unsafe(no_mangle)]
    pub extern "C" fn typvia_background_round() -> bool {
        let Some(app) = APP.get() else {
            return false;
        };
        crate::run_sync_round(app);
        true
    }
}

/// Android: the WorkManager worker may run in a process the system started
/// without any activity — `Application.onCreate` runs, Tauri does not. The
/// entry therefore assembles the minimum host state itself:
/// database, secure store over the application context, sync host. The
/// shared pieces (`SyncHost::start`, `run_round`, `temporary_expire`) are
/// the same functions the foreground path calls.
#[cfg(target_os = "android")]
pub(crate) mod android {
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Duration;

    use jni::JNIEnv;
    use jni::objects::{JObject, JString};
    use jni::sys::jint;

    use crate::secure_store::android::HeadlessKeystore;
    use crate::sync::SyncHost;

    /// Worker result protocol (SyncWorker.kt holds the same table):
    /// the round ran or there was legitimately nothing to do.
    const ROUND_OK: jint = 0;
    /// A transient obstacle (database busy, host assembly failed) worth a
    /// WorkManager retry with backoff inside this period.
    const ROUND_RETRY: jint = 1;

    /// The foreground app holds the same database; a short wait then
    /// yielding the round (retry) keeps the single-writer discipline
    /// without ever stalling the worker thread for long.
    const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

    /// JNI entry for `SyncWorker.nativeBackgroundRound(applicationContext)`
    /// (an instance method — the receiver arrives as the second argument).
    /// Never panics across the boundary and never logs — codes only.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_dev_typvia_mobile_SyncWorker_nativeBackgroundRound<'local>(
        mut env: JNIEnv<'local>,
        _this: JObject<'local>,
        context: JObject<'local>,
    ) -> jint {
        let Some(data_dir) = data_dir(&mut env, &context) else {
            return ROUND_RETRY;
        };
        let Some(store) = HeadlessKeystore::from_worker_call(&mut env, &context) else {
            return ROUND_RETRY;
        };
        run_headless_round(&data_dir, &store)
    }

    /// `Context.getDataDir()` — the same directory Tauri's `app_data_dir()`
    /// resolves to on Android, so the worker opens the
    /// database the foreground app owns.
    fn data_dir(env: &mut JNIEnv<'_>, context: &JObject<'_>) -> Option<PathBuf> {
        let file = env
            .call_method(context, "getDataDir", "()Ljava/io/File;", &[])
            .ok()?
            .l()
            .ok()?;
        let path = env
            .call_method(&file, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        let path: String = env.get_string(&JString::from(path)).ok()?.into();
        Some(PathBuf::from(path))
    }

    /// The headless assembly: open + migrate, expiry sweep, one round.
    /// Assembly failures are RETRY (busy or transient); a round that cannot
    /// reach the server is the offline state and still ROUND_OK — the next
    /// period tries again.
    fn run_headless_round(data_dir: &std::path::Path, store: &HeadlessKeystore) -> jint {
        let Ok(mut conn) = typvia_core::db::open(&data_dir.join("typvia.db")) else {
            return ROUND_RETRY;
        };
        if conn.busy_timeout(BUSY_TIMEOUT).is_err()
            || typvia_core::db::migrate_to_latest(&mut conn).is_err()
        {
            return ROUND_RETRY;
        }
        let Ok(now) = crate::commands::now_ms() else {
            return ROUND_RETRY;
        };
        let Ok(device_id) = typvia_host_service::service::load_or_create_device_id(data_dir) else {
            return ROUND_RETRY;
        };
        let mut host = SyncHost::start(
            &conn,
            store,
            device_id,
            typvia_host_service::service::current_platform(),
            now,
        );
        // The background wake-up doubles as the temporary-snippet expiry
        // sweep, mirroring the foreground tick.
        let _ = typvia_host_service::service::temporary_expire(&conn, now);
        let db = Mutex::new(conn);
        let _ = crate::sync::run_round(&db, store, &mut host, now);
        ROUND_OK
    }
}
