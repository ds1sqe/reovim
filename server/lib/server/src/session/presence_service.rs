//! `PresenceService` - session-owned presence graph authority.
//!
//! Owns the per-session presence graph while leaving transport/event behavior on
//! `Session` and the gRPC layer. This keeps presence state distinct from client
//! membership and editing-relation routing.

use super::{ClientId, ClientPresence, PresenceMap};

/// Session-owned service for presence graph state.
#[derive(Debug, Default)]
pub struct PresenceService {
    map: PresenceMap,
}

impl PresenceService {
    /// Create an empty presence service.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add client to the presence graph and return pre-existing peers.
    pub fn join(&self, presence: ClientPresence) -> Vec<ClientPresence> {
        self.map.join(presence)
    }

    /// Remove client from the presence graph.
    pub fn leave(&self, client_id: ClientId) -> Option<ClientPresence> {
        self.map.leave(client_id)
    }

    /// Update client presence and return the new snapshot.
    pub fn update<F>(&self, client_id: ClientId, f: F) -> Option<ClientPresence>
    where
        F: FnOnce(&mut ClientPresence),
    {
        self.map.update(client_id, f)
    }

    /// Get a client's current presence snapshot.
    #[must_use]
    pub fn get(&self, client_id: ClientId) -> Option<ClientPresence> {
        self.map.get(client_id)
    }

    /// List all connected clients in the presence graph.
    #[must_use]
    pub fn list(&self) -> Vec<ClientPresence> {
        self.map.list()
    }

    /// Return all followers of the target client.
    #[must_use]
    pub fn followers_of(&self, target_id: ClientId) -> Vec<ClientId> {
        self.map.followers_of(target_id)
    }

    /// Check if a client is present.
    #[must_use]
    pub fn contains(&self, client_id: ClientId) -> bool {
        self.map.contains(client_id)
    }

    /// Count present clients.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check whether the presence graph is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
