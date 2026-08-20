//! Change-notification seam for sync.
//!
//! Core stays unaware of sealing and transports: write use cases only
//! announce "entity X changed / was deleted" through [`notify_change`],
//! inside their own write transaction. The sync layer registers a
//! [`ChangeObserver`] per database connection that turns the announcement
//! into a sealed outbox record within the same transaction (seal-at-write;
//! a failure aborts the whole write — no silent divergence). With no
//! observer registered — sync disabled or never set up — notification is a
//! single map lookup and a no-op.
//!
//! Registration is keyed by the underlying SQLite handle address, which is
//! stable for the connection's lifetime (the `Connection` value itself may
//! move). Entries are per-connection, so parallel tests with their own
//! connections never share observer state; the returned guard unregisters
//! on drop.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

use rusqlite::Connection;

use crate::model::{SyncEntityType, TimestampMs};

/// One announced entity change. `deleted_at` distinguishes deletion (the
/// sync layer seals a tombstone) from creation/update (full-state record).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityChange {
    pub entity_type: SyncEntityType,
    pub entity_id: String,
    /// Set when the entity was deleted; the deletion time in ms UTC.
    pub deleted_at: Option<TimestampMs>,
    /// The clock of the write use case (record `updated_at` source).
    pub now: TimestampMs,
}

/// Error surfaced by an observer. Carries only the structural cause chain;
/// observers never put entity content into error messages (log red line).
#[derive(Debug)]
pub struct HookError(Box<dyn std::error::Error + Send + Sync>);

impl HookError {
    pub fn new(cause: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self(Box::new(cause))
    }
}

impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sync change hook failed: {}", self.0)
    }
}

impl std::error::Error for HookError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// Receives entity-change announcements inside the write transaction.
pub trait ChangeObserver: Send + Sync {
    /// Called with the transaction's connection; any writes the observer
    /// performs join the surrounding transaction. An error aborts the
    /// caller's write (fail-closed: a write that cannot be queued must not
    /// silently diverge from the account).
    fn entity_changed(&self, conn: &Connection, change: &EntityChange) -> Result<(), HookError>;
}

/// Per-connection observer registry. A poisoned lock only means another
/// thread panicked mid-insert/remove; the map itself stays valid.
static REGISTRY: OnceLock<Mutex<HashMap<usize, Arc<dyn ChangeObserver>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<usize, Arc<dyn ChangeObserver>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_registry() -> std::sync::MutexGuard<'static, HashMap<usize, Arc<dyn ChangeObserver>>> {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The registry key: the SQLite handle address, stable for the connection's
/// lifetime. The pointer is never dereferenced.
fn connection_key(conn: &Connection) -> usize {
    // SAFETY: `handle` only returns the raw pointer; obtaining it has no
    // side effects and the pointer is used purely as an opaque map key.
    unsafe { conn.handle() as usize }
}

/// Unregisters its connection's observer on drop, so a dropped sync engine
/// can never leave a stale observer behind on a reused handle address.
pub struct ObserverGuard {
    key: usize,
}

impl Drop for ObserverGuard {
    fn drop(&mut self) {
        lock_registry().remove(&self.key);
    }
}

impl fmt::Debug for ObserverGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObserverGuard").finish_non_exhaustive()
    }
}

/// Registers `observer` for `conn`, replacing any previous registration.
/// Keep the guard alive for as long as the observer should receive changes.
#[must_use = "dropping the guard unregisters the observer"]
pub fn register_observer(conn: &Connection, observer: Arc<dyn ChangeObserver>) -> ObserverGuard {
    let key = connection_key(conn);
    lock_registry().insert(key, observer);
    ObserverGuard { key }
}

/// Announces an entity change to the connection's observer, if any. Called
/// by write use cases inside their write transaction; a no-op without an
/// observer.
pub fn notify_change(conn: &Connection, change: &EntityChange) -> Result<(), HookError> {
    let observer = lock_registry().get(&connection_key(conn)).cloned();
    match observer {
        Some(observer) => observer.entity_changed(conn, change),
        None => Ok(()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::db::open_in_memory;

    struct Counting {
        calls: AtomicUsize,
    }

    impl ChangeObserver for Counting {
        fn entity_changed(
            &self,
            _conn: &Connection,
            _change: &EntityChange,
        ) -> Result<(), HookError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn change() -> EntityChange {
        EntityChange {
            entity_type: SyncEntityType::Snippet,
            entity_id: "s1".to_string(),
            deleted_at: None,
            now: 1_700_000_000_000,
        }
    }

    #[test]
    fn notify_without_an_observer_is_a_no_op() {
        let conn = open_in_memory().unwrap();
        assert!(notify_change(&conn, &change()).is_ok());
    }

    #[test]
    fn a_registered_observer_receives_changes_until_the_guard_drops() {
        let conn = open_in_memory().unwrap();
        let observer = Arc::new(Counting {
            calls: AtomicUsize::new(0),
        });
        let guard = register_observer(&conn, observer.clone());
        notify_change(&conn, &change()).unwrap();
        notify_change(&conn, &change()).unwrap();
        assert_eq!(observer.calls.load(Ordering::SeqCst), 2);

        drop(guard);
        notify_change(&conn, &change()).unwrap();
        assert_eq!(observer.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn observers_are_scoped_to_their_own_connection() {
        let with_observer = open_in_memory().unwrap();
        let without_observer = open_in_memory().unwrap();
        let observer = Arc::new(Counting {
            calls: AtomicUsize::new(0),
        });
        let _guard = register_observer(&with_observer, observer.clone());
        notify_change(&without_observer, &change()).unwrap();
        assert_eq!(observer.calls.load(Ordering::SeqCst), 0);
        notify_change(&with_observer, &change()).unwrap();
        assert_eq!(observer.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn the_transaction_connection_maps_to_the_same_observer() {
        let conn = open_in_memory().unwrap();
        let observer = Arc::new(Counting {
            calls: AtomicUsize::new(0),
        });
        let _guard = register_observer(&conn, observer.clone());
        let tx = conn.unchecked_transaction().unwrap();
        notify_change(&tx, &change()).unwrap();
        tx.commit().unwrap();
        assert_eq!(observer.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn observer_errors_propagate_to_the_caller() {
        struct Failing;
        impl ChangeObserver for Failing {
            fn entity_changed(
                &self,
                _conn: &Connection,
                _change: &EntityChange,
            ) -> Result<(), HookError> {
                Err(HookError::new(std::io::Error::other("queue unavailable")))
            }
        }
        let conn = open_in_memory().unwrap();
        let _guard = register_observer(&conn, Arc::new(Failing));
        assert!(notify_change(&conn, &change()).is_err());
    }
}
