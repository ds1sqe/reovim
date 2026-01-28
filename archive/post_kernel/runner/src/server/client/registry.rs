//! Client registry for tracking connected clients per session.
//!
//! Uses lock-free patterns (`ArcSwap`) for efficient client lookup.

use std::{collections::HashMap, sync::Arc};

use reovim_arch::sync::ArcSwap;

use {super::client::Client, crate::session::ClientId};

/// Registry of clients connected to a session.
///
/// Each session has its own `ClientRegistry` tracking connected clients.
/// Uses `ArcSwap` for lock-free reads (client lookup, broadcast iteration)
/// with RCU pattern for mutations (client add/remove).
///
/// # Example
///
/// ```ignore
/// use runner::client::{Client, ClientRegistry};
///
/// let registry = ClientRegistry::new();
///
/// // Add a client
/// registry.insert(client);
///
/// // Lock-free lookup
/// if let Some(client) = registry.get(&client_id) {
///     client.send_line("hello").await?;
/// }
///
/// // Broadcast to all clients
/// for client in registry.iter() {
///     client.send_line("broadcast message").await?;
/// }
/// ```
pub struct ClientRegistry {
    /// Clients indexed by ID, using `ArcSwap` for lock-free reads.
    clients: ArcSwap<HashMap<ClientId, Arc<Client>>>,
}

impl ClientRegistry {
    /// Create a new empty client registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            clients: ArcSwap::from_pointee(HashMap::new()),
        }
    }

    /// Get a client by ID (lock-free).
    #[must_use]
    pub fn get(&self, id: &ClientId) -> Option<Arc<Client>> {
        self.clients.load().get(id).cloned()
    }

    /// Insert a client into the registry.
    ///
    /// Uses RCU pattern for thread-safe mutation.
    pub fn insert(&self, client: &Arc<Client>) {
        self.clients.rcu(|current| {
            let mut new = (**current).clone();
            new.insert(client.id(), Arc::clone(client));
            new
        });
    }

    /// Remove a client from the registry.
    ///
    /// Returns the removed client if it existed.
    pub fn remove(&self, id: &ClientId) -> Option<Arc<Client>> {
        let mut removed = None;
        self.clients.rcu(|current| {
            let mut new = (**current).clone();
            removed = new.remove(id);
            new
        });
        removed
    }

    /// Check if a client is in the registry.
    #[must_use]
    pub fn contains(&self, id: &ClientId) -> bool {
        self.clients.load().contains_key(id)
    }

    /// Get the number of connected clients.
    #[must_use]
    pub fn len(&self) -> usize {
        self.clients.load().len()
    }

    /// Check if there are no connected clients.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clients.load().is_empty()
    }

    /// Get all client IDs.
    pub fn client_ids(&self) -> Vec<ClientId> {
        self.clients.load().keys().copied().collect()
    }

    /// Iterate over all clients.
    ///
    /// Creates a snapshot - clients added/removed during iteration
    /// won't be reflected.
    pub fn iter(&self) -> impl Iterator<Item = Arc<Client>> {
        let snapshot = self.clients.load();
        snapshot.values().cloned().collect::<Vec<_>>().into_iter()
    }
}

impl Default for ClientRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ClientRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let clients = self.clients.load();
        f.debug_struct("ClientRegistry")
            .field("client_count", &clients.len())
            .field("clients", &clients.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: We can't easily create Client instances without TCP sockets.
    // Unit tests focus on registry structure.

    #[test]
    fn test_client_registry_new() {
        let registry = ClientRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_client_registry_contains_empty() {
        let registry = ClientRegistry::new();
        let id = ClientId::new(1);

        assert!(!registry.contains(&id));
    }

    #[test]
    fn test_client_registry_client_ids_empty() {
        let registry = ClientRegistry::new();
        assert!(registry.client_ids().is_empty());
    }
}
