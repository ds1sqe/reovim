//! Multi-client presence tracking (Phase 14, Epic #465).
//!
//! Tracks connected clients and their presence state for multi-client awareness.
//!
//! # Architecture
//!
//! ```text
//! PresenceMap (per-session)
//!   └─ HashMap<ClientId, ClientPresence>
//!       └─ cursor, viewport, mode, sync_mode
//! ```
//!
//! # Thread Safety
//!
//! `PresenceMap` uses `RwLock` for interior mutability:
//! - Reads are non-blocking for other reads
//! - Writes are quick (`HashMap` operations)
//!
//! # Example
//!
//! ```ignore
//! let map = PresenceMap::new();
//!
//! // Client joins
//! let presence = ClientPresence::new(client_id, "tui", "laptop");
//! let peers = map.join(presence);
//!
//! // Update cursor
//! map.update(client_id, |p| p.cursor = new_position);
//!
//! // Client leaves
//! map.leave(client_id);
//! ```

use std::{collections::HashMap, time::SystemTime};

use parking_lot::RwLock;

use super::id::ClientId;

/// Synchronization mode for a client.
///
/// Determines how a client interacts with presence updates from other clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncMode {
    /// Own cursor, own scroll. Receives updates but ignores them.
    #[default]
    Independent,

    /// Follow another client's cursor and scroll position.
    Follow {
        /// Client ID being followed.
        target: ClientId,
    },

    /// Presenting mode. Normal editing, tagged so others can follow.
    Present,
}

/// Presence state for a single connected client.
///
/// Domain-neutral (#753 E6): cursor, `visible_lines`, and mode are domain-owned
/// state — they live in the domain driver's per-client `EditingState`, not here.
/// The server presence layer only tracks connection identity and sync topology.
#[derive(Debug, Clone)]
pub struct ClientPresence {
    /// Unique client ID (assigned by server on Join).
    pub client_id: ClientId,

    /// Client type identifier ("tui", "android", "web", "cli").
    pub client_type: String,

    /// User-friendly display name ("laptop", "phone", "tablet").
    pub display_name: String,

    /// Current buffer being viewed (domain-neutral buffer ID).
    pub buffer_id: Option<usize>,

    /// Sync mode.
    pub sync_mode: SyncMode,

    /// When the client joined.
    pub joined_at: SystemTime,
}

impl ClientPresence {
    /// Create a new client presence with default state.
    #[must_use]
    pub fn new(
        client_id: ClientId,
        client_type: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            client_id,
            client_type: client_type.into(),
            display_name: display_name.into(),
            buffer_id: None,
            sync_mode: SyncMode::default(),
            joined_at: SystemTime::now(),
        }
    }

    /// Get the joined timestamp as Unix milliseconds.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // u128 millis to u64 - safe for next 500M years
    pub fn joined_at_ms(&self) -> u64 {
        self.joined_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    }
}

/// Thread-safe presence map for a session.
///
/// Tracks all connected clients and their presence state.
///
/// # Complexity
///
/// | Operation | Complexity | Notes |
/// |-----------|------------|-------|
/// | `join()` | O(n) | Collects existing peers |
/// | `leave()` | O(1) | HashMap remove |
/// | `update()` | O(1) | HashMap get_mut |
/// | `get()` | O(1) | HashMap get |
/// | `list()` | O(n) | Collects all |
/// | `followers_of()` | O(n) | Scans all clients |
#[derive(Debug, Default)]
pub struct PresenceMap {
    clients: RwLock<HashMap<ClientId, ClientPresence>>,
}

impl PresenceMap {
    /// Create a new empty presence map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add client to presence map, returns list of existing peers.
    ///
    /// The returned list contains all clients that were connected *before*
    /// this client joined (excluding the new client).
    pub fn join(&self, presence: ClientPresence) -> Vec<ClientPresence> {
        let mut clients = self.clients.write();
        let peers: Vec<_> = clients.values().cloned().collect();
        clients.insert(presence.client_id, presence);
        peers
    }

    /// Remove client from presence map, returns removed presence if found.
    pub fn leave(&self, client_id: ClientId) -> Option<ClientPresence> {
        self.clients.write().remove(&client_id)
    }

    /// Update client presence via closure, returns updated presence if found.
    ///
    /// Returns `None` if client is not in the map.
    pub fn update<F>(&self, client_id: ClientId, f: F) -> Option<ClientPresence>
    where
        F: FnOnce(&mut ClientPresence),
    {
        let mut clients = self.clients.write();
        clients.get_mut(&client_id).map(|presence| {
            f(presence);
            presence.clone()
        })
    }

    /// Get client presence by ID.
    #[must_use]
    pub fn get(&self, client_id: ClientId) -> Option<ClientPresence> {
        self.clients.read().get(&client_id).cloned()
    }

    /// List all connected clients.
    #[must_use]
    pub fn list(&self) -> Vec<ClientPresence> {
        self.clients.read().values().cloned().collect()
    }

    /// Get all clients following a specific target.
    ///
    /// Returns client IDs of all clients whose `sync_mode` is `Follow { target }`.
    #[must_use]
    pub fn followers_of(&self, target_id: ClientId) -> Vec<ClientId> {
        self.clients
            .read()
            .values()
            .filter_map(|p| match p.sync_mode {
                SyncMode::Follow { target } if target == target_id => Some(p.client_id),
                _ => None,
            })
            .collect()
    }

    /// Check if a client exists in the map.
    #[must_use]
    pub fn contains(&self, client_id: ClientId) -> bool {
        self.clients.read().contains_key(&client_id)
    }

    /// Get count of connected clients.
    #[must_use]
    pub fn len(&self) -> usize {
        self.clients.read().len()
    }

    /// Check if no clients are connected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clients.read().is_empty()
    }
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod tests;
