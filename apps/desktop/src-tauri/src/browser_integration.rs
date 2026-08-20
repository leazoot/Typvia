//! Desktop browser-integration data plane: an opt-in switch and the
//! browser-snapshot directory the native-messaging host reads. The
//! extension host never touches the main database — this module is the only
//! producer of what it may see, and it reuses the shared snapshot writer
//! (sensitive snippets contribute an opaque ciphertext envelope only).

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use typvia_host_service::error::IpcError;

/// Snapshot subdirectory of the app data dir the extension host reads.
const SNAPSHOT_DIR_NAME: &str = "browser-snapshot";
/// Presence of this marker file is the whole on/off state: no database row,
/// so the host binary and the desktop app agree without sharing the DB.
const ENABLED_MARKER_NAME: &str = "browser-integration.enabled";

/// Reports whether the user has switched browser integration on.
pub fn is_enabled(data_dir: &Path) -> bool {
    data_dir.join(ENABLED_MARKER_NAME).is_file()
}

/// Resolves (creating on demand) the browser-snapshot directory.
pub fn snapshot_dir(data_dir: &Path) -> Result<PathBuf, IpcError> {
    let dir = data_dir.join(SNAPSHOT_DIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|_| IpcError::system())?;
    Ok(dir)
}

/// Switches integration on: writes the first snapshot before flipping the
/// marker, so an enabled integration always has a snapshot to serve; then
/// registers the native-messaging host manifests for installed browsers.
/// Manifest install is best-effort — a missing host binary leaves the data
/// plane on and `host_installed` false, never a failed switch.
pub fn enable(conn: &Connection, data_dir: &Path, now: i64) -> Result<(), IpcError> {
    enable_with(
        conn,
        data_dir,
        now,
        home_dir().as_deref(),
        resolve_host_binary().as_deref(),
    )
}

/// Injection point so tests never touch the real user profile.
fn enable_with(
    conn: &Connection,
    data_dir: &Path,
    now: i64,
    home: Option<&Path>,
    host: Option<&Path>,
) -> Result<(), IpcError> {
    write_current_snapshot(conn, data_dir, now)?;
    std::fs::write(data_dir.join(ENABLED_MARKER_NAME), b"").map_err(|_| IpcError::system())?;
    if let (Some(home), Some(host)) = (home, host) {
        // Failures here (permissions, exotic layouts) leave a partial set;
        // `host_installed` reports the truth and disable cleans up.
        let _ = manifests::install(home, data_dir, host);
    }
    Ok(())
}

/// Switches integration off and removes every host manifest this app wrote.
/// Idempotent: disabling an already-disabled integration succeeds. The last
/// snapshot file stays in place (it holds nothing the extension was not
/// already allowed to read); `sync` freezes it while disabled.
pub fn disable(data_dir: &Path) -> Result<(), IpcError> {
    disable_with(data_dir, home_dir().as_deref())
}

/// Injection point so tests never touch the real user profile.
fn disable_with(data_dir: &Path, home: Option<&Path>) -> Result<(), IpcError> {
    if let Some(home) = home {
        manifests::remove(home, data_dir);
    }
    match std::fs::remove_file(data_dir.join(ENABLED_MARKER_NAME)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(IpcError::system()),
    }
}

/// Whether at least one host manifest is currently registered.
pub fn host_installed() -> bool {
    home_dir().is_some_and(|home| manifests::any_present(&home))
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// Locates the `typvia-browser-host` binary: env override, then a sibling
/// of the running executable (the packaged layout once bundling lands; also
/// the dev layout when both binaries are built into the same target dir).
fn resolve_host_binary() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("TYPVIA_BROWSER_HOST_BIN") {
        let path = std::path::PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let sibling = std::env::current_exe()
        .ok()?
        .parent()?
        .join(manifests::HOST_BINARY_NAME);
    sibling.is_file().then_some(sibling)
}

/// Native-messaging host manifest registration. macOS/Linux: per-browser JSON
/// files under the user profile. Windows: one manifest file in the app
/// data dir plus HKCU registry values pointing at it (the platform's
/// native-messaging convention). Chromium-family manifests carry
/// `allowed_origins` (the stable extension id from the committed manifest
/// `key`); Firefox carries `allowed_extensions` (the gecko id).
mod manifests {
    use std::path::Path;
    #[cfg(not(windows))]
    use std::path::PathBuf;

    use typvia_host_service::error::IpcError;

    /// Host name the extension connects to; also the manifest file stem.
    pub const HOST_NAME: &str = "dev.typvia.browser_host";
    #[cfg(not(windows))]
    pub const HOST_BINARY_NAME: &str = "typvia-browser-host";
    #[cfg(windows)]
    pub const HOST_BINARY_NAME: &str = "typvia-browser-host.exe";
    /// Stable Chromium extension id derived from the committed `key` field
    /// in apps/browser-extension/manifest.json.
    const CHROMIUM_EXTENSION_ID: &str = "gpnfboeojngaliicpijloghfknhcahch";
    /// Gecko id from apps/browser-extension/manifest.firefox.json.
    const FIREFOX_EXTENSION_ID: &str = "extension@typvia.dev";

    enum Family {
        Chromium,
        Firefox,
    }

    fn manifest_json(family: &Family, host_path: &Path) -> serde_json::Value {
        let base = serde_json::json!({
            "name": HOST_NAME,
            "description": "Typvia browser extension host (read-only snapshot access)",
            "path": host_path,
            "type": "stdio",
        });
        let mut manifest = base;
        match family {
            Family::Chromium => {
                manifest["allowed_origins"] =
                    serde_json::json!([format!("chrome-extension://{CHROMIUM_EXTENSION_ID}/")]);
            }
            Family::Firefox => {
                manifest["allowed_extensions"] = serde_json::json!([FIREFOX_EXTENSION_ID]);
            }
        }
        manifest
    }

    /// Browser profile roots relative to `home`, macOS then Linux layouts.
    /// A manifest is written only when the browser root itself exists, so
    /// enabling never litters config trees of browsers that are not there.
    #[cfg(not(windows))]
    fn targets(home: &Path) -> Vec<(PathBuf, PathBuf, Family)> {
        let chromium =
            |root: &str, dir: &str| (home.join(root), home.join(root).join(dir), Family::Chromium);
        vec![
            // macOS
            chromium(
                "Library/Application Support/Google/Chrome",
                "NativeMessagingHosts",
            ),
            chromium(
                "Library/Application Support/Chromium",
                "NativeMessagingHosts",
            ),
            chromium(
                "Library/Application Support/Microsoft Edge",
                "NativeMessagingHosts",
            ),
            (
                home.join("Library/Application Support/Mozilla"),
                home.join("Library/Application Support/Mozilla/NativeMessagingHosts"),
                Family::Firefox,
            ),
            // Linux
            chromium(".config/google-chrome", "NativeMessagingHosts"),
            chromium(".config/chromium", "NativeMessagingHosts"),
            chromium(".config/microsoft-edge", "NativeMessagingHosts"),
            (
                home.join(".mozilla"),
                home.join(".mozilla/native-messaging-hosts"),
                Family::Firefox,
            ),
        ]
    }

    /// Writes the manifest into every installed browser's host directory.
    #[cfg(not(windows))]
    pub fn install(home: &Path, _data_dir: &Path, host_path: &Path) -> Result<(), IpcError> {
        for (root, dir, family) in targets(home) {
            if !root.is_dir() {
                continue;
            }
            std::fs::create_dir_all(&dir).map_err(|_| IpcError::system())?;
            let json = serde_json::to_vec_pretty(&manifest_json(&family, host_path))
                .map_err(|_| IpcError::system())?;
            std::fs::write(dir.join(format!("{HOST_NAME}.json")), json)
                .map_err(|_| IpcError::system())?;
        }
        Ok(())
    }

    /// Removes every manifest this app may have written (best-effort).
    #[cfg(not(windows))]
    pub fn remove(home: &Path, _data_dir: &Path) {
        for (_, dir, _) in targets(home) {
            let _ = std::fs::remove_file(dir.join(format!("{HOST_NAME}.json")));
        }
    }

    /// True when at least one written manifest is still in place.
    #[cfg(not(windows))]
    pub fn any_present(home: &Path) -> bool {
        targets(home)
            .iter()
            .any(|(_, dir, _)| dir.join(format!("{HOST_NAME}.json")).is_file())
    }

    /// Windows registry roots (HKCU) whose `NativeMessagingHosts\<name>`
    /// default value points at the manifest file.
    #[cfg(windows)]
    const REGISTRY_KEYS: [(&str, Family); 4] = [
        (
            "Software\\Google\\Chrome\\NativeMessagingHosts",
            Family::Chromium,
        ),
        ("Software\\Chromium\\NativeMessagingHosts", Family::Chromium),
        (
            "Software\\Microsoft\\Edge\\NativeMessagingHosts",
            Family::Chromium,
        ),
        ("Software\\Mozilla\\NativeMessagingHosts", Family::Firefox),
    ];

    /// Windows: two manifest files (Chromium/Firefox flavors) in the app
    /// data dir, referenced from HKCU per browser. Registration is
    /// per-user — no elevation, mirroring the unix per-profile writes.
    #[cfg(windows)]
    pub fn install(_home: &Path, data_dir: &Path, host_path: &Path) -> Result<(), IpcError> {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let dir = data_dir.join("browser-manifests");
        std::fs::create_dir_all(&dir).map_err(|_| IpcError::system())?;
        let chromium_path = dir.join(format!("{HOST_NAME}.chromium.json"));
        let firefox_path = dir.join(format!("{HOST_NAME}.firefox.json"));
        for (family, path) in [
            (Family::Chromium, &chromium_path),
            (Family::Firefox, &firefox_path),
        ] {
            let json = serde_json::to_vec_pretty(&manifest_json(&family, host_path))
                .map_err(|_| IpcError::system())?;
            std::fs::write(path, json).map_err(|_| IpcError::system())?;
        }
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for (root, family) in REGISTRY_KEYS {
            let (key, _) = hkcu
                .create_subkey(format!("{root}\\{HOST_NAME}"))
                .map_err(|_| IpcError::system())?;
            let manifest = match family {
                Family::Chromium => &chromium_path,
                Family::Firefox => &firefox_path,
            };
            let manifest_path = manifest.to_string_lossy().into_owned();
            key.set_value("", &manifest_path)
                .map_err(|_| IpcError::system())?;
        }
        Ok(())
    }

    /// Windows removal: registry values then the manifest files.
    #[cfg(windows)]
    pub fn remove(_home: &Path, data_dir: &Path) {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for (root, _) in REGISTRY_KEYS {
            let _ = hkcu.delete_subkey_all(format!("{root}\\{HOST_NAME}"));
        }
        let dir = data_dir.join("browser-manifests");
        let _ = std::fs::remove_file(dir.join(format!("{HOST_NAME}.chromium.json")));
        let _ = std::fs::remove_file(dir.join(format!("{HOST_NAME}.firefox.json")));
    }

    /// Windows: at least one registry registration is still in place.
    #[cfg(windows)]
    pub fn any_present(_home: &Path) -> bool {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        REGISTRY_KEYS
            .iter()
            .any(|(root, _)| hkcu.open_subkey(format!("{root}\\{HOST_NAME}")).is_ok())
    }

    #[cfg(all(test, not(windows)))]
    #[allow(clippy::unwrap_used)]
    pub mod tests {
        use super::*;

        fn fake_home_with(browsers: &[&str]) -> tempfile::TempDir {
            let home = tempfile::tempdir().unwrap();
            for root in browsers {
                std::fs::create_dir_all(home.path().join(root)).unwrap();
            }
            home
        }

        #[test]
        fn installs_only_into_existing_browser_roots_with_the_right_shape() {
            let home = fake_home_with(&[
                "Library/Application Support/Google/Chrome",
                "Library/Application Support/Mozilla",
            ]);
            let host = Path::new("/opt/typvia/typvia-browser-host");

            let data = tempfile::tempdir().unwrap();
            install(home.path(), data.path(), host).unwrap();

            let chrome_manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(home.path().join(
                    "Library/Application Support/Google/Chrome/NativeMessagingHosts/dev.typvia.browser_host.json",
                ))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(chrome_manifest["name"], HOST_NAME);
            assert_eq!(chrome_manifest["type"], "stdio");
            assert_eq!(
                chrome_manifest["allowed_origins"][0],
                format!("chrome-extension://{CHROMIUM_EXTENSION_ID}/")
            );
            assert!(chrome_manifest.get("allowed_extensions").is_none());

            let firefox_manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(home.path().join(
                    "Library/Application Support/Mozilla/NativeMessagingHosts/dev.typvia.browser_host.json",
                ))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(
                firefox_manifest["allowed_extensions"][0],
                FIREFOX_EXTENSION_ID
            );
            assert!(firefox_manifest.get("allowed_origins").is_none());

            // Browsers that are not installed stay untouched.
            assert!(!home.path().join(".config/chromium").exists());
            assert!(any_present(home.path()));
        }

        #[test]
        fn remove_deletes_every_manifest_and_is_idempotent() {
            let home = fake_home_with(&["Library/Application Support/Google/Chrome", ".mozilla"]);
            let data = tempfile::tempdir().unwrap();
            install(home.path(), data.path(), Path::new("/opt/host")).unwrap();
            assert!(any_present(home.path()));

            remove(home.path(), data.path());
            assert!(!any_present(home.path()));
            // A second removal over nothing is fine.
            remove(home.path(), data.path());
        }

        #[test]
        fn a_reinstall_overwrites_with_the_new_host_path() {
            let home = fake_home_with(&[".config/google-chrome"]);
            let data = tempfile::tempdir().unwrap();
            install(home.path(), data.path(), Path::new("/old/host")).unwrap();
            install(home.path(), data.path(), Path::new("/new/host")).unwrap();

            let manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(home.path().join(
                    ".config/google-chrome/NativeMessagingHosts/dev.typvia.browser_host.json",
                ))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(manifest["path"], "/new/host");
        }
    }
}

/// Refreshes the snapshot after a data change. A successful no-op while
/// integration is off — mirrors the espanso sync contract, so callers can
/// poke unconditionally.
pub fn sync(conn: &Connection, data_dir: &Path, now: i64) -> Result<(), IpcError> {
    if !is_enabled(data_dir) {
        return Ok(());
    }
    write_current_snapshot(conn, data_dir, now)
}

fn write_current_snapshot(conn: &Connection, data_dir: &Path, now: i64) -> Result<(), IpcError> {
    let dir = snapshot_dir(data_dir)?;
    let device_id = typvia_host_service::service::load_or_create_device_id(data_dir)?;
    typvia_host_service::snapshot::write_snapshot(conn, &dir, &device_id, now)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use typvia_core::vault::VaultSession;
    use typvia_host_service::dto::SnippetCreateInput;
    use typvia_host_service::service;

    const NOW: i64 = 1_700_000_000_000;

    fn test_conn() -> Connection {
        let mut conn = typvia_core::db::open_in_memory().unwrap();
        typvia_core::db::migrate_to_latest(&mut conn).unwrap();
        conn
    }

    fn create_normal(conn: &Connection, title: &str, body: &str) {
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
            NOW,
        )
        .unwrap();
    }

    fn snapshot_bytes(data_dir: &Path) -> Vec<u8> {
        std::fs::read(
            data_dir
                .join(SNAPSHOT_DIR_NAME)
                .join(typvia_host_service::snapshot::SNAPSHOT_FILE_NAME),
        )
        .unwrap()
    }

    #[test]
    fn enable_writes_a_snapshot_and_flips_the_marker() {
        let conn = test_conn();
        let data_dir = tempfile::tempdir().unwrap();
        create_normal(&conn, "Tail logs", "docker logs -f app");

        let home = tempfile::tempdir().unwrap();
        enable_with(&conn, data_dir.path(), NOW, Some(home.path()), None).unwrap();

        assert!(is_enabled(data_dir.path()));
        let bytes = snapshot_bytes(data_dir.path());
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("docker logs -f app"));
    }

    #[test]
    fn sync_is_a_successful_no_op_while_disabled() {
        let conn = test_conn();
        let data_dir = tempfile::tempdir().unwrap();
        create_normal(&conn, "Tail logs", "docker logs -f app");

        sync(&conn, data_dir.path(), NOW).unwrap();

        assert!(!is_enabled(data_dir.path()));
        assert!(
            !data_dir
                .path()
                .join(SNAPSHOT_DIR_NAME)
                .join(typvia_host_service::snapshot::SNAPSHOT_FILE_NAME)
                .exists()
        );
    }

    #[test]
    fn sync_refreshes_the_snapshot_while_enabled() {
        let conn = test_conn();
        let data_dir = tempfile::tempdir().unwrap();
        create_normal(&conn, "First", "first body");
        let home = tempfile::tempdir().unwrap();
        enable_with(&conn, data_dir.path(), NOW, Some(home.path()), None).unwrap();

        create_normal(&conn, "Second", "second body");
        sync(&conn, data_dir.path(), NOW + 1_000).unwrap();

        let text = String::from_utf8(snapshot_bytes(data_dir.path())).unwrap();
        assert!(text.contains("second body"));
    }

    #[test]
    fn disable_is_idempotent_and_freezes_the_snapshot() {
        let conn = test_conn();
        let data_dir = tempfile::tempdir().unwrap();
        create_normal(&conn, "First", "first body");
        let home = tempfile::tempdir().unwrap();
        enable_with(&conn, data_dir.path(), NOW, Some(home.path()), None).unwrap();

        disable_with(data_dir.path(), Some(home.path())).unwrap();
        disable_with(data_dir.path(), Some(home.path())).unwrap();
        assert!(!is_enabled(data_dir.path()));

        // The stale file stays but no further data flows into it.
        create_normal(&conn, "Second", "second body");
        sync(&conn, data_dir.path(), NOW + 1_000).unwrap();
        let text = String::from_utf8(snapshot_bytes(data_dir.path())).unwrap();
        assert!(!text.contains("second body"));
    }

    #[test]
    fn sensitive_title_and_body_never_reach_the_snapshot_bytes() {
        // Red line: the snapshot the extension host reads may carry
        // sensitive rows only as opaque ciphertext.
        let conn = test_conn();
        let data_dir = tempfile::tempdir().unwrap();
        let mut session = VaultSession::new();
        session
            .initialize(&conn, b"correct horse battery staple", NOW)
            .unwrap();
        let title = "REDLINE_TITLE_MARKER";
        let secret = "REDLINE_BODY_FAKE_sk_51_DO_NOT_LEAK";
        service::vault_create_secret(
            &conn,
            &session,
            SnippetCreateInput {
                title: title.to_string(),
                body: secret.to_string(),
                snippet_type: "sensitive".to_string(),
                description: None,
                folder_id: None,
                trigger: None,
                trigger_mode: None,
                language: None,
            },
            NOW,
        )
        .unwrap();
        create_normal(&conn, "Normal", "plain body stays visible");

        let home = tempfile::tempdir().unwrap();
        enable_with(&conn, data_dir.path(), NOW, Some(home.path()), None).unwrap();

        let bytes = snapshot_bytes(data_dir.path());
        let window_hit = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert!(!window_hit(title.as_bytes()));
        assert!(!window_hit(secret.as_bytes()));
        // Self-check that the scan sees plaintext at all (scan validity).
        assert!(window_hit(b"plain body stays visible"));
    }
}
