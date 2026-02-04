//! Presence tracking for multi-client awareness.
//!
//! Tracks which clients are connected and their current state.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::wire::ClientPresence;

/// Tracker for client presence in a session.
///
/// Maintains a map of connected clients and their state,
/// enabling collaborative features and awareness.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PresenceTracker {
    clients: HashMap<String, ClientPresence>,
}

impl PresenceTracker {
    /// Create a new empty presence tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a client's presence.
    pub fn upsert(&mut self, presence: ClientPresence) {
        self.clients.insert(presence.client_id.clone(), presence);
    }

    /// Remove a client from tracking.
    ///
    /// Returns the removed presence, if found.
    pub fn remove(&mut self, client_id: &str) -> Option<ClientPresence> {
        self.clients.remove(client_id)
    }

    /// Get a client's presence.
    #[must_use]
    pub fn get(&self, client_id: &str) -> Option<&ClientPresence> {
        self.clients.get(client_id)
    }

    /// Get a mutable reference to a client's presence.
    #[must_use]
    pub fn get_mut(&mut self, client_id: &str) -> Option<&mut ClientPresence> {
        self.clients.get_mut(client_id)
    }

    /// Check if a client is present.
    #[must_use]
    pub fn contains(&self, client_id: &str) -> bool {
        self.clients.contains_key(client_id)
    }

    /// Get all client IDs.
    pub fn client_ids(&self) -> impl Iterator<Item = &str> {
        self.clients.keys().map(String::as_str)
    }

    /// Get all presences.
    pub fn all(&self) -> impl Iterator<Item = &ClientPresence> {
        self.clients.values()
    }

    /// Get the number of tracked clients.
    #[must_use]
    pub fn len(&self) -> usize {
        self.clients.len()
    }

    /// Check if no clients are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    /// Get clients viewing a specific buffer.
    pub fn clients_in_buffer(&self, buffer_id: u64) -> impl Iterator<Item = &ClientPresence> {
        self.clients
            .values()
            .filter(move |p| p.active_buffer == Some(buffer_id))
    }

    /// Get active (non-idle) clients.
    pub fn active_clients(&self) -> impl Iterator<Item = &ClientPresence> {
        self.clients.values().filter(|p| p.is_active)
    }

    /// Get broadcasting clients.
    pub fn broadcasters(&self) -> impl Iterator<Item = &ClientPresence> {
        self.clients
            .values()
            .filter(|p| p.sync_mode.is_broadcasting())
    }

    /// Clear all presences.
    pub fn clear(&mut self) {
        self.clients.clear();
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::wire::SyncMode};

    fn test_presence(id: &str) -> ClientPresence {
        ClientPresence::new(id)
    }

    #[test]
    fn test_presence_tracker_new() {
        let tracker = PresenceTracker::new();
        assert!(tracker.is_empty());
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn test_presence_tracker_upsert() {
        let mut tracker = PresenceTracker::new();

        tracker.upsert(test_presence("client-1"));
        assert_eq!(tracker.len(), 1);
        assert!(tracker.contains("client-1"));

        // Update existing
        let mut updated = test_presence("client-1");
        updated.is_active = false;
        tracker.upsert(updated);
        assert_eq!(tracker.len(), 1);
        assert!(!tracker.get("client-1").unwrap().is_active);
    }

    #[test]
    fn test_presence_tracker_remove() {
        let mut tracker = PresenceTracker::new();
        tracker.upsert(test_presence("client-1"));

        let removed = tracker.remove("client-1");
        assert!(removed.is_some());
        assert!(tracker.is_empty());

        let removed = tracker.remove("nonexistent");
        assert!(removed.is_none());
    }

    #[test]
    fn test_presence_tracker_get() {
        let mut tracker = PresenceTracker::new();
        tracker.upsert(test_presence("client-1"));

        assert!(tracker.get("client-1").is_some());
        assert!(tracker.get("nonexistent").is_none());
    }

    #[test]
    fn test_presence_tracker_client_ids() {
        let mut tracker = PresenceTracker::new();
        tracker.upsert(test_presence("a"));
        tracker.upsert(test_presence("b"));
        tracker.upsert(test_presence("c"));

        let ids: Vec<_> = tracker.client_ids().collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
    }

    #[test]
    fn test_presence_tracker_clients_in_buffer() {
        let mut tracker = PresenceTracker::new();
        tracker.upsert(test_presence("a").with_position(1, 0, 0));
        tracker.upsert(test_presence("b").with_position(2, 0, 0));
        tracker.upsert(test_presence("c").with_position(1, 10, 5));

        assert_eq!(tracker.clients_in_buffer(1).count(), 2);
    }

    #[test]
    fn test_presence_tracker_active_clients() {
        let mut tracker = PresenceTracker::new();
        tracker.upsert(test_presence("active-1"));
        tracker.upsert(test_presence("active-2"));

        let mut inactive = test_presence("inactive");
        inactive.is_active = false;
        tracker.upsert(inactive);

        assert_eq!(tracker.active_clients().count(), 2);
    }

    #[test]
    fn test_presence_tracker_broadcasters() {
        let mut tracker = PresenceTracker::new();
        tracker.upsert(test_presence("normal"));
        tracker.upsert(test_presence("broadcaster").with_sync_mode(SyncMode::Broadcast));

        let broadcasters: Vec<_> = tracker.broadcasters().collect();
        assert_eq!(broadcasters.len(), 1);
        assert_eq!(broadcasters[0].client_id, "broadcaster");
    }

    #[test]
    fn test_presence_tracker_clear() {
        let mut tracker = PresenceTracker::new();
        tracker.upsert(test_presence("a"));
        tracker.upsert(test_presence("b"));
        assert_eq!(tracker.len(), 2);

        tracker.clear();
        assert!(tracker.is_empty());
    }
}
