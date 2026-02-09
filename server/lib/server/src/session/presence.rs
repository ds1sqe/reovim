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
#[derive(Debug, Clone)]
pub struct ClientPresence {
    /// Unique client ID (assigned by server on Join).
    pub client_id: ClientId,

    /// Client type identifier ("tui", "android", "web", "cli").
    pub client_type: String,

    /// User-friendly display name ("laptop", "phone", "tablet").
    pub display_name: String,

    /// Current buffer being viewed.
    pub buffer_id: Option<usize>,

    /// Cursor position in buffer (line, column).
    pub cursor: (usize, usize),

    /// Visible line range (start, end exclusive).
    pub visible_lines: (usize, usize),

    /// Current mode name ("NORMAL", "INSERT", etc.).
    pub mode: String,

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
            cursor: (0, 0),
            visible_lines: (0, 24),
            mode: "NORMAL".to_string(),
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
mod tests {
    use super::*;

    fn make_presence(id: usize, name: &str) -> ClientPresence {
        ClientPresence::new(ClientId::new(id), "tui", name)
    }

    #[test]
    fn test_presence_map_new() {
        let map = PresenceMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_join_returns_empty_peers_first() {
        let map = PresenceMap::new();
        let presence = make_presence(1, "laptop");

        let peers = map.join(presence);

        assert!(peers.is_empty());
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_join_returns_existing_peers() {
        let map = PresenceMap::new();

        map.join(make_presence(1, "laptop"));
        let peers = map.join(make_presence(2, "phone"));

        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].client_id, ClientId::new(1));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_join_multiple_clients() {
        let map = PresenceMap::new();

        map.join(make_presence(1, "laptop"));
        map.join(make_presence(2, "phone"));
        let peers = map.join(make_presence(3, "tablet"));

        assert_eq!(peers.len(), 2);
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn test_leave_removes_client() {
        let map = PresenceMap::new();
        let client_id = ClientId::new(1);

        map.join(make_presence(1, "laptop"));
        let removed = map.leave(client_id);

        assert!(removed.is_some());
        assert_eq!(removed.unwrap().display_name, "laptop");
        assert!(map.is_empty());
    }

    #[test]
    fn test_leave_returns_none_for_unknown() {
        let map = PresenceMap::new();

        let removed = map.leave(ClientId::new(999));

        assert!(removed.is_none());
    }

    #[test]
    fn test_update_modifies_presence() {
        let map = PresenceMap::new();
        let client_id = ClientId::new(1);

        map.join(make_presence(1, "laptop"));
        let updated = map.update(client_id, |p| {
            p.cursor = (10, 5);
            p.mode = "INSERT".to_string();
        });

        assert!(updated.is_some());
        let presence = updated.unwrap();
        assert_eq!(presence.cursor, (10, 5));
        assert_eq!(presence.mode, "INSERT");

        // Verify it was actually persisted
        let stored = map.get(client_id).unwrap();
        assert_eq!(stored.cursor, (10, 5));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_update_returns_none_for_unknown() {
        let map = PresenceMap::new();

        let updated = map.update(ClientId::new(999), |p| {
            p.cursor = (10, 5);
        });

        assert!(updated.is_none());
    }

    #[test]
    fn test_get_returns_presence() {
        let map = PresenceMap::new();
        let client_id = ClientId::new(1);

        map.join(make_presence(1, "laptop"));
        let presence = map.get(client_id);

        assert!(presence.is_some());
        assert_eq!(presence.unwrap().display_name, "laptop");
    }

    #[test]
    fn test_get_returns_none_for_unknown() {
        let map = PresenceMap::new();

        let presence = map.get(ClientId::new(999));

        assert!(presence.is_none());
    }

    #[test]
    fn test_list_returns_all_clients() {
        let map = PresenceMap::new();

        map.join(make_presence(1, "laptop"));
        map.join(make_presence(2, "phone"));
        map.join(make_presence(3, "tablet"));

        let clients = map.list();
        assert_eq!(clients.len(), 3);
    }

    #[test]
    fn test_followers_of_returns_following_clients() {
        let map = PresenceMap::new();
        let presenter_id = ClientId::new(1);
        let follower1_id = ClientId::new(2);
        let follower2_id = ClientId::new(3);

        // Set up presenter
        map.join(make_presence(1, "presenter"));
        map.update(presenter_id, |p| p.sync_mode = SyncMode::Present);

        // Set up followers
        map.join(make_presence(2, "follower1"));
        map.update(follower1_id, |p| {
            p.sync_mode = SyncMode::Follow {
                target: presenter_id,
            };
        });

        map.join(make_presence(3, "follower2"));
        map.update(follower2_id, |p| {
            p.sync_mode = SyncMode::Follow {
                target: presenter_id,
            };
        });

        // Independent client
        map.join(make_presence(4, "independent"));

        let followers = map.followers_of(presenter_id);

        assert_eq!(followers.len(), 2);
        assert!(followers.contains(&follower1_id));
        assert!(followers.contains(&follower2_id));
    }

    #[test]
    fn test_followers_of_empty_when_no_followers() {
        let map = PresenceMap::new();

        map.join(make_presence(1, "client1"));
        map.join(make_presence(2, "client2"));

        let followers = map.followers_of(ClientId::new(1));

        assert!(followers.is_empty());
    }

    #[test]
    fn test_contains() {
        let map = PresenceMap::new();
        let client_id = ClientId::new(1);

        assert!(!map.contains(client_id));

        map.join(make_presence(1, "laptop"));
        assert!(map.contains(client_id));

        map.leave(client_id);
        assert!(!map.contains(client_id));
    }

    #[test]
    fn test_sync_mode_default() {
        let presence = make_presence(1, "laptop");
        assert_eq!(presence.sync_mode, SyncMode::Independent);
    }

    #[test]
    fn test_client_presence_new() {
        let presence = ClientPresence::new(ClientId::new(42), "android", "phone");

        assert_eq!(presence.client_id, ClientId::new(42));
        assert_eq!(presence.client_type, "android");
        assert_eq!(presence.display_name, "phone");
        assert_eq!(presence.buffer_id, None);
        assert_eq!(presence.cursor, (0, 0));
        assert_eq!(presence.visible_lines, (0, 24));
        assert_eq!(presence.mode, "NORMAL");
        assert_eq!(presence.sync_mode, SyncMode::Independent);
    }

    #[test]
    fn test_joined_at_ms() {
        let presence = make_presence(1, "laptop");
        let ms = presence.joined_at_ms();

        // Should be a reasonable timestamp (after year 2020)
        assert!(ms > 1_577_836_800_000); // Jan 1, 2020
    }

    #[test]
    fn test_concurrent_join_leave() {
        use std::{sync::Arc, thread};

        let map = Arc::new(PresenceMap::new());
        let mut handles = Vec::new();

        // Spawn 10 threads that join and leave
        for i in 0..10 {
            let map_clone = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                let presence = make_presence(i, &format!("client{i}"));
                map_clone.join(presence);

                // Do some operations
                let _ = map_clone.get(ClientId::new(i));
                let _ = map_clone.update(ClientId::new(i), |p| p.cursor = (i, i));
                let _ = map_clone.list();

                // Leave
                map_clone.leave(ClientId::new(i));
            }));
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // All clients should have left
        assert!(map.is_empty());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_sync_mode_follow() {
        let presenter_id = ClientId::new(1);
        let mode = SyncMode::Follow {
            target: presenter_id,
        };

        if let SyncMode::Follow { target } = mode {
            assert_eq!(target, presenter_id);
        } else {
            panic!("Expected Follow mode");
        }
    }

    #[test]
    fn test_sync_mode_present() {
        let mode = SyncMode::Present;
        assert_eq!(mode, SyncMode::Present);
    }
}
