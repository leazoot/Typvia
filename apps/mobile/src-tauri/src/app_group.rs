//! Resolves the shared App Group container the keyboard extension reads
//! snapshots from. iOS only: other hosts have no App Group
//! concept, so resolution reports `None` and the snapshot pipeline stays
//! off (unit tests exercise the writer against plain temp directories).

use std::path::PathBuf;

/// The formal App Group shared by the host app and the keyboard extension.
/// Must match the `com.apple.security.application-groups` entitlement of
/// every target that touches the snapshot. iOS-gated with its only caller
/// so non-iOS builds carry no dead constant.
#[cfg(target_os = "ios")]
pub const APP_GROUP_ID: &str = "group.dev.typvia.mobile";

/// Returns the App Group container directory when the OS grants one.
/// `None` means the entitlement is missing or the platform has no container.
#[cfg(target_os = "ios")]
pub fn container_dir() -> Option<PathBuf> {
    use objc2_foundation::{NSFileManager, NSString};

    let group = NSString::from_str(APP_GROUP_ID);
    let url = NSFileManager::defaultManager()
        .containerURLForSecurityApplicationGroupIdentifier(&group)?;
    let path = url.path()?;
    Some(PathBuf::from(path.to_string()))
}

/// Non-iOS hosts (the dev shell, unit tests) have no App Group container.
#[cfg(not(target_os = "ios"))]
pub fn container_dir() -> Option<PathBuf> {
    None
}
