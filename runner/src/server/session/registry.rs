//! Session registry for managing multiple sessions.
//!
//! Uses lock-free patterns (`ArcSwap`) for hot-path session lookup,
//! following the concurrency model in `docs/reference/concurrency.md`.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use reovim_arch::sync::ArcSwap;

use super::{SessionId, id::ClientId, session::Session};

/// Registry of active sessions with lock-free lookup.
///
/// The server maintains a single `SessionRegistry` that tracks all active
/// sessions. Multiple clients can connect to the same session.
///
/// # Concurrency Model
///
/// Following the lock hierarchy from `docs/reference/concurrency.md`:
/// - **Level 0 (Lock-Free)**: Session lookup via `ArcSwap`
/// - **Level 0 (Lock-Free)**: Client ID generation via `AtomicU64`
///
/// This design prioritizes:
/// - Lock-free reads for session lookup (hot path)
/// - RCU (Read-Copy-Update) pattern for session mutations (cold path)
///
/// # Performance Targets
///
/// | Operation | Target | Pattern |
/// |-----------|--------|---------|
/// | Session lookup | <100ns | `ArcSwap` lock-free read |
/// | Client ID gen | <10ns | `AtomicU64` `fetch_add` |
///
/// # Example
///
/// ```ignore
/// use runner::session::{SessionRegistry, Session, SessionId};
///
/// let registry = SessionRegistry::new();
///
/// // Create and insert a session
/// let session = Session::new(...);
/// registry.insert(session.clone());
///
/// // Lock-free lookup (hot path)
/// if let Some(session) = registry.get(&SessionId::default()) {
///     // Use session...
/// }
///
/// // Generate unique client ID (lock-free)
/// let client_id = registry.next_client_id();
/// ```
pub struct SessionRegistry {
    /// Sessions indexed by ID, using `ArcSwap` for lock-free reads.
    ///
    /// Uses RCU pattern: reads are lock-free via `load()`,
    /// writes copy-on-write via `rcu()`.
    sessions: ArcSwap<HashMap<SessionId, Arc<Session>>>,

    /// Counter for generating unique client IDs.
    ///
    /// Using `Relaxed` ordering because client IDs just need uniqueness,
    /// not synchronization with other operations.
    next_client_id: AtomicU64,
}

impl SessionRegistry {
    /// Create a new empty session registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: ArcSwap::from_pointee(HashMap::new()),
            next_client_id: AtomicU64::new(1), // Start at 1, 0 could be "no client"
        }
    }

    /// Get a session by ID (lock-free).
    ///
    /// This is the hot-path operation for client requests.
    /// Returns `None` if the session doesn't exist.
    #[must_use]
    pub fn get(&self, id: &SessionId) -> Option<Arc<Session>> {
        self.sessions.load().get(id).cloned()
    }

    /// Insert a session into the registry.
    ///
    /// Uses RCU (Read-Copy-Update) pattern:
    /// 1. Clone current state
    /// 2. Modify clone
    /// 3. Atomically swap
    ///
    /// If a session with the same ID exists, it is replaced.
    pub fn insert(&self, session: &Arc<Session>) {
        self.sessions.rcu(|current| {
            let mut new = (**current).clone();
            new.insert(session.id().clone(), Arc::clone(session));
            new
        });
    }

    /// Remove a session from the registry.
    ///
    /// Uses RCU pattern. Returns `true` if a session was removed.
    pub fn remove(&self, id: &SessionId) -> bool {
        let mut removed = false;
        self.sessions.rcu(|current| {
            let mut new = (**current).clone();
            removed = new.remove(id).is_some();
            new
        });
        removed
    }

    /// Get or create a session.
    ///
    /// Returns the existing session if one exists with the given ID,
    /// or creates a new one using the provided closure.
    ///
    /// Note: This is NOT atomic. In a race condition, multiple sessions
    /// might be created. For a single-threaded startup this is fine.
    /// For concurrent session creation, consider using `get_or_insert`.
    pub fn get_or_create<F>(&self, id: &SessionId, create: F) -> Arc<Session>
    where
        F: FnOnce() -> Arc<Session>,
    {
        if let Some(session) = self.get(id) {
            return session;
        }

        let session = create();
        self.insert(&session);
        session
    }

    /// Generate a unique client ID (lock-free).
    ///
    /// Client IDs are globally unique across all sessions.
    /// Uses `Relaxed` ordering as we only need uniqueness.
    #[must_use]
    pub fn next_client_id(&self) -> ClientId {
        ClientId::new(self.next_client_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Check if a session exists.
    #[must_use]
    pub fn contains(&self, id: &SessionId) -> bool {
        self.sessions.load().contains_key(id)
    }

    /// Get the number of active sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.load().len()
    }

    /// Check if there are no active sessions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.load().is_empty()
    }

    /// Get all session IDs.
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.sessions.load().keys().cloned().collect()
    }

    /// Iterate over all sessions.
    ///
    /// Note: This creates a snapshot. Sessions added/removed during
    /// iteration won't be reflected.
    pub fn iter(&self) -> impl Iterator<Item = Arc<Session>> {
        let snapshot = self.sessions.load();
        snapshot.values().cloned().collect::<Vec<_>>().into_iter()
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SessionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sessions = self.sessions.load();
        f.debug_struct("SessionRegistry")
            .field("session_count", &sessions.len())
            .field("sessions", &sessions.keys().collect::<Vec<_>>())
            .field("next_client_id", &self.next_client_id.load(Ordering::Relaxed))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId};

    use super::*;

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn create_test_session(name: &str) -> Arc<Session> {
        Session::new(SessionId::new(name), KernelContext::default(), test_mode_id())
    }

    #[test]
    fn test_session_registry_new() {
        let registry = SessionRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_session_registry_insert_get() {
        let registry = SessionRegistry::new();
        let session = create_test_session("test");
        let id = session.id().clone();

        registry.insert(&session);

        assert!(registry.contains(&id));
        assert_eq!(registry.len(), 1);

        let retrieved = registry.get(&id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id().name(), "test");
    }

    #[test]
    fn test_session_registry_remove() {
        let registry = SessionRegistry::new();
        let session = create_test_session("test");
        let id = session.id().clone();

        registry.insert(&session);
        assert!(registry.contains(&id));

        let removed = registry.remove(&id);
        assert!(removed);
        assert!(!registry.contains(&id));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_session_registry_remove_nonexistent() {
        let registry = SessionRegistry::new();
        let id = SessionId::new("nonexistent");

        let removed = registry.remove(&id);
        assert!(!removed);
    }

    #[test]
    fn test_session_registry_get_or_create() {
        let registry = SessionRegistry::new();
        let id = SessionId::new("default");

        // First call creates
        let session1 = registry.get_or_create(&id, || create_test_session("default"));
        assert_eq!(session1.id().name(), "default");

        // Second call returns existing
        let session2 =
            registry.get_or_create(&id, || panic!("Should not be called - session exists"));

        // Same Arc
        assert!(Arc::ptr_eq(&session1, &session2));
    }

    #[test]
    fn test_session_registry_next_client_id() {
        let registry = SessionRegistry::new();

        let id1 = registry.next_client_id();
        let id2 = registry.next_client_id();
        let id3 = registry.next_client_id();

        // IDs should be unique and sequential
        assert_eq!(id1.value(), 1);
        assert_eq!(id2.value(), 2);
        assert_eq!(id3.value(), 3);
    }

    #[test]
    fn test_session_registry_multiple_sessions() {
        let registry = SessionRegistry::new();

        registry.insert(&create_test_session("session1"));
        registry.insert(&create_test_session("session2"));
        registry.insert(&create_test_session("session3"));

        assert_eq!(registry.len(), 3);

        let ids = registry.session_ids();
        assert!(ids.contains(&SessionId::new("session1")));
        assert!(ids.contains(&SessionId::new("session2")));
        assert!(ids.contains(&SessionId::new("session3")));
    }

    #[test]
    fn test_session_registry_iter() {
        let registry = SessionRegistry::new();

        registry.insert(&create_test_session("a"));
        registry.insert(&create_test_session("b"));

        assert_eq!(registry.iter().count(), 2);
    }

    #[test]
    fn test_session_registry_replace() {
        let registry = SessionRegistry::new();

        // Insert first session
        let session1 = create_test_session("test");
        registry.insert(&session1);

        // Insert second session with same ID
        let session2 = create_test_session("test");
        registry.insert(&session2);

        // Should still have only one session
        assert_eq!(registry.len(), 1);

        // Should be the second session
        let retrieved = registry.get(&SessionId::new("test")).unwrap();
        assert!(Arc::ptr_eq(&retrieved, &session2));
    }
}
