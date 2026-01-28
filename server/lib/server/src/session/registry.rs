//! Session registry for managing multiple sessions.
//!
//! Uses lock-free patterns (`ArcSwap`) for hot-path session lookup.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use arc_swap::ArcSwap;

use super::{Session, SessionId, id::ClientId};

/// Registry of active sessions with lock-free lookup.
///
/// # Concurrency Model
///
/// - **Level 0 (Lock-Free)**: Session lookup via `ArcSwap`
/// - **Level 0 (Lock-Free)**: Client ID generation via `AtomicUsize`
pub struct SessionRegistry {
    /// Sessions indexed by ID, using `ArcSwap` for lock-free reads.
    sessions: ArcSwap<HashMap<SessionId, Arc<Session>>>,

    /// Counter for generating unique client IDs.
    next_client_id: AtomicUsize,
}

impl SessionRegistry {
    /// Create a new empty session registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: ArcSwap::from_pointee(HashMap::new()),
            next_client_id: AtomicUsize::new(1),
        }
    }

    /// Get a session by ID (lock-free).
    #[must_use]
    pub fn get(&self, id: &SessionId) -> Option<Arc<Session>> {
        self.sessions.load().get(id).cloned()
    }

    /// Insert or replace a session.
    pub fn insert(&self, session: &Arc<Session>) {
        let session = Arc::clone(session);
        self.sessions.rcu(|current| {
            let mut new = (**current).clone();
            new.insert(session.id().clone(), Arc::clone(&session));
            new
        });
    }

    /// Remove a session by ID.
    pub fn remove(&self, id: &SessionId) -> Option<Arc<Session>> {
        let mut removed = None;
        self.sessions.rcu(|current| {
            let mut new = (**current).clone();
            removed = new.remove(id);
            new
        });
        removed
    }

    /// List all session IDs.
    #[must_use]
    pub fn list(&self) -> Vec<SessionId> {
        self.sessions.load().keys().cloned().collect()
    }

    /// Generate a unique client ID (lock-free).
    pub fn next_client_id(&self) -> ClientId {
        ClientId::new(self.next_client_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the number of sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sessions.load().len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.load().is_empty()
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_insert_get() {
        let registry = SessionRegistry::new();
        let session = Arc::new(Session::new(SessionId::new("test")));

        registry.insert(&session);

        let found = registry.get(&SessionId::new("test"));
        assert!(found.is_some());
    }

    #[test]
    fn test_registry_next_client_id() {
        let registry = SessionRegistry::new();

        let id1 = registry.next_client_id();
        let id2 = registry.next_client_id();

        assert_ne!(id1, id2);
    }
}
