//! `ClientDirectory` - client membership and relation authority.
//!
//! Owns the connected-client map plus editing-relation validation and input
//! target resolution. `Session` remains the composition root and transport host,
//! but forwards membership/routing work to this directory.

use std::collections::HashMap;

use {
    parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    reovim_driver_session::ExtensionMap,
};

use super::{Client, ClientId, ClientRelation, EditingState, TransitionResult};

/// Authoritative owner for connected clients and editing relations.
pub struct ClientDirectory {
    clients: RwLock<HashMap<ClientId, Client>>,
}

impl ClientDirectory {
    /// Create an empty client directory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
        }
    }

    /// Borrow the client map immutably.
    pub fn read(&self) -> RwLockReadGuard<'_, HashMap<ClientId, Client>> {
        self.clients.read()
    }

    /// Borrow the client map mutably.
    pub fn write(&self) -> RwLockWriteGuard<'_, HashMap<ClientId, Client>> {
        self.clients.write()
    }

    /// Insert a fully initialized client.
    pub fn add_client_with_state(&self, client: Client) {
        let mut clients = self.clients.write();
        clients.insert(client.id, client);
    }

    /// Remove a client, optionally running logic while the directory lock is held.
    pub fn remove_client_with<F>(&self, client_id: ClientId, before_remove: F) -> Option<Client>
    where
        F: FnOnce(&Client),
    {
        let mut clients = self.clients.write();
        if let Some(client) = clients.get(&client_id) {
            before_remove(client);
        }
        clients.remove(&client_id)
    }

    /// Get a cloned client.
    #[must_use]
    pub fn get_client(&self, client_id: ClientId) -> Option<Client> {
        let clients = self.clients.read();
        clients.get(&client_id).cloned()
    }

    /// Set a client's relation with validation.
    pub fn set_client_relation(
        &self,
        client_id: ClientId,
        relation: Option<ClientRelation>,
    ) -> Result<(), TransitionResult> {
        let mut clients = self.clients.write();

        let validation_result = {
            let Some(client) = clients.get(&client_id) else {
                return Err(TransitionResult::TargetNotFound(client_id));
            };
            Client::validate_relation_change(client, relation, &clients)
        };

        let result = match validation_result {
            TransitionResult::Ok => {
                if let Some(client) = clients.get_mut(&client_id) {
                    client.set_relation_unchecked(relation);
                }
                Ok(())
            }
            other => Err(other),
        };

        drop(clients);
        result
    }

    /// Set a client's relation without validation.
    pub fn set_client_relation_unchecked(
        &self,
        client_id: ClientId,
        relation: Option<ClientRelation>,
    ) -> bool {
        let mut clients = self.clients.write();
        clients.get_mut(&client_id).is_some_and(|client| {
            client.set_relation_unchecked(relation);
            true
        })
    }

    /// Sync cursor and set relation.
    pub fn sync_and_set_relation(
        &self,
        client_id: ClientId,
        target_id: ClientId,
        relation: Option<ClientRelation>,
    ) -> Result<(), TransitionResult> {
        let mut clients = self.clients.write();

        let target_cursor = clients
            .get(&target_id)
            .and_then(|c| c.state.windows.active())
            .map(|w| w.cursor);

        if let (Some(cursor), Some(client)) = (target_cursor, clients.get_mut(&client_id))
            && let Some(window) = client.state.windows.active_mut()
        {
            window.cursor = cursor;
        }

        let validation_result = {
            let Some(client) = clients.get(&client_id) else {
                return Err(TransitionResult::TargetNotFound(client_id));
            };
            Client::validate_relation_change(client, relation, &clients)
        };

        let result = match validation_result {
            TransitionResult::Ok => {
                if let Some(client) = clients.get_mut(&client_id) {
                    client.set_relation_unchecked(relation);
                }
                Ok(())
            }
            other => Err(other),
        };

        drop(clients);
        result
    }

    /// Get the effective editing state for a client.
    #[must_use]
    pub fn client_state(&self, client_id: ClientId) -> Option<EditingState> {
        let clients = self.clients.read();
        clients
            .get(&client_id)
            .and_then(|c| c.effective_state(&clients))
            .cloned()
    }

    /// Update a client's effective editing state via closure.
    pub fn update_client_state<F>(&self, client_id: ClientId, f: F) -> bool
    where
        F: FnOnce(&mut EditingState),
    {
        let mut clients = self.clients.write();

        let Some(client) = clients.get(&client_id) else {
            return false;
        };

        let target_id = match client.relation {
            None => client_id,
            Some(ClientRelation::Sharing { with }) => with,
            Some(ClientRelation::Following { .. }) => return false,
        };

        if let Some(target_client) = clients.get_mut(&target_id) {
            f(&mut target_client.state);
            true
        } else {
            false
        }
    }

    /// Execute a closure with read access to the clients map.
    pub fn with_clients<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<ClientId, Client>) -> R,
    {
        let clients = self.clients.read();
        f(&clients)
    }

    /// Execute a closure with write access to the clients map.
    pub fn with_clients_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut HashMap<ClientId, Client>) -> R,
    {
        let mut clients = self.clients.write();
        f(&mut clients)
    }

    /// Run a closure on a client's effective extension map without cloning.
    pub fn with_client_extensions<F, R>(&self, client_id: ClientId, f: F) -> Option<R>
    where
        F: FnOnce(&ExtensionMap) -> R,
    {
        let clients = self.clients.read();
        let client = clients.get(&client_id)?;
        let state = client.effective_state(&clients)?;
        let result = f(&state.extensions);
        drop(clients);
        Some(result)
    }

    /// Run a closure with mutable access to an input-target extension map.
    pub fn with_client_extensions_mut<F, R>(&self, client_id: ClientId, f: F) -> Option<R>
    where
        F: FnOnce(&mut ExtensionMap) -> R,
    {
        let mut clients = self.clients.write();
        let target_id = Self::find_input_target(&clients, client_id)?;
        let target_client = clients.get_mut(&target_id)?;
        let result = f(&mut target_client.state.extensions);
        drop(clients);
        Some(result)
    }

    /// Count connected clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.read().len()
    }

    /// Check if a client is connected.
    #[must_use]
    pub fn has_client(&self, client_id: ClientId) -> bool {
        self.clients.read().contains_key(&client_id)
    }

    /// List connected client IDs in sorted order.
    #[must_use]
    pub fn connected_client_ids(&self) -> Vec<ClientId> {
        let mut ids: Vec<_> = self.clients.read().keys().copied().collect();
        ids.sort_unstable_by_key(ClientId::as_usize);
        ids
    }

    /// Find the target client ID for input routing.
    #[must_use]
    pub fn find_input_target(
        clients: &HashMap<ClientId, Client>,
        client_id: ClientId,
    ) -> Option<ClientId> {
        let client = clients.get(&client_id)?;
        if client.is_independent() {
            Some(client_id)
        } else if client.is_sharing() {
            client.target_id()
        } else {
            None
        }
    }
}
