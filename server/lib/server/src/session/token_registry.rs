//! Token registry for session-based client authentication (#483).
//!
//! Maps opaque session tokens to client IDs. The gRPC interceptor reads
//! `x-reovim-token` from request metadata, resolves it here, and injects
//! the `ClientId` into request extensions.
//!
//! # Concurrency Model
//!
//! Uses a single `RwLock` guarding both the forward (token→client) and
//! reverse (client→token) maps. This guarantees consistency between the
//! two maps — they are always updated atomically.
//!
//! Optimized for reads: the hot path (interceptor lookup) takes a read lock.

use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;

use super::id::ClientId;

/// Opaque session token issued on `Join()`.
///
/// 128 bits of random entropy, formatted as 32 hex characters.
/// Cheap to clone (`Arc<str>` interior).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionToken(Arc<str>);

impl SessionToken {
    /// Generate a new random token (128 bits of entropy).
    #[must_use]
    pub fn generate() -> Self {
        Self(format!("{:032x}", rand::random::<u128>()).into())
    }

    /// Get the token as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SessionToken {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Interior state guarded by a single `RwLock`.
struct TokenMaps {
    /// token → `ClientId` (primary: used by interceptor on every request).
    forward: HashMap<SessionToken, ClientId>,
    /// `ClientId` → token (reverse: used by revoke-by-client on Leave/disconnect).
    reverse: HashMap<ClientId, SessionToken>,
}

/// Bidirectional token-to-client mapping.
///
/// Thread-safe. The interceptor calls [`resolve`](Self::resolve) on the hot
/// path (read lock only). Mutations (`register`, `revoke`) take write locks
/// but are infrequent (Join/Leave).
pub struct TokenRegistry {
    maps: RwLock<TokenMaps>,
}

impl TokenRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            maps: RwLock::new(TokenMaps {
                forward: HashMap::new(),
                reverse: HashMap::new(),
            }),
        }
    }

    /// Register a new token for a client. Returns the generated token.
    ///
    /// If the client already has a token, the old token is revoked first.
    pub fn register(&self, client_id: ClientId) -> SessionToken {
        let token = SessionToken::generate();
        let mut maps = self.maps.write();

        // Revoke any existing token for this client
        if let Some(old_token) = maps.reverse.remove(&client_id) {
            maps.forward.remove(&old_token);
        }

        maps.forward.insert(token.clone(), client_id);
        maps.reverse.insert(client_id, token.clone());
        token
    }

    /// Resolve a token to its `ClientId`. O(1).
    ///
    /// This is the hot path — called by the interceptor on every gRPC request.
    #[must_use]
    pub fn resolve(&self, token: &SessionToken) -> Option<ClientId> {
        self.maps.read().forward.get(token).copied()
    }

    /// Revoke a specific token. Returns the associated `ClientId` if found.
    pub fn revoke(&self, token: &SessionToken) -> Option<ClientId> {
        let mut maps = self.maps.write();
        let client_id = maps.forward.remove(token)?;
        maps.reverse.remove(&client_id);
        drop(maps);
        Some(client_id)
    }

    /// Revoke a client's token by `ClientId`. Returns the revoked token.
    ///
    /// O(1) via reverse map. Idempotent — returns `None` if no token found.
    pub fn revoke_by_client(&self, client_id: ClientId) -> Option<SessionToken> {
        let mut maps = self.maps.write();
        let token = maps.reverse.remove(&client_id)?;
        maps.forward.remove(&token);
        drop(maps);
        Some(token)
    }

    /// Number of active tokens (for diagnostics).
    #[must_use]
    pub fn len(&self) -> usize {
        self.maps.read().forward.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.maps.read().forward.is_empty()
    }
}

impl Default for TokenRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_generates_unique_tokens() {
        let registry = TokenRegistry::new();
        let t1 = registry.register(ClientId::new(1));
        let t2 = registry.register(ClientId::new(2));
        assert_ne!(t1, t2);
        assert_eq!(t1.as_str().len(), 32); // 128-bit hex
    }

    #[test]
    fn resolve_returns_correct_client() {
        let registry = TokenRegistry::new();
        let token = registry.register(ClientId::new(42));
        assert_eq!(registry.resolve(&token), Some(ClientId::new(42)));
    }

    #[test]
    fn resolve_unknown_token_returns_none() {
        let registry = TokenRegistry::new();
        let fake = SessionToken::from("nonexistent");
        assert_eq!(registry.resolve(&fake), None);
    }

    #[test]
    fn revoke_removes_mapping() {
        let registry = TokenRegistry::new();
        let token = registry.register(ClientId::new(1));

        let removed = registry.revoke(&token);
        assert_eq!(removed, Some(ClientId::new(1)));
        assert_eq!(registry.resolve(&token), None);
        assert!(registry.is_empty());
    }

    #[test]
    fn revoke_by_client_removes_mapping() {
        let registry = TokenRegistry::new();
        let token = registry.register(ClientId::new(7));

        let removed = registry.revoke_by_client(ClientId::new(7));
        assert_eq!(removed, Some(token));
        assert!(registry.is_empty());
    }

    #[test]
    fn revoke_by_client_idempotent() {
        let registry = TokenRegistry::new();
        registry.register(ClientId::new(1));

        assert!(registry.revoke_by_client(ClientId::new(1)).is_some());
        assert!(registry.revoke_by_client(ClientId::new(1)).is_none());
    }

    #[test]
    fn register_replaces_existing_token() {
        let registry = TokenRegistry::new();
        let old_token = registry.register(ClientId::new(1));
        let new_token = registry.register(ClientId::new(1));

        // Old token should be invalidated
        assert_eq!(registry.resolve(&old_token), None);
        // New token should work
        assert_eq!(registry.resolve(&new_token), Some(ClientId::new(1)));
        // Only one entry
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn len_and_is_empty() {
        let registry = TokenRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(ClientId::new(1));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);

        registry.register(ClientId::new(2));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn session_token_display() {
        let token = SessionToken::from("test-token-123");
        assert_eq!(format!("{token}"), "test-token-123");
    }

    #[test]
    fn default_creates_empty_registry() {
        let registry = TokenRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }
}
