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
#[path = "presence_tests.rs"]
mod tests;
