//! Extension state bridge system for server-to-client state streaming.
//!
//! This module provides the [`ExtensionStateBridge`] trait that adapts session
//! extension state (e.g., `CmdlineState`) into JSON for gRPC transmission to
//! clients. Modules register bridges via [`BridgeProvider`] during `init()`,
//! and bootstrap collects them into a [`BridgeRegistry`] at startup.
//!
//! # Architecture (#514, #468)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │ SESSION DRIVER (bridges/)                                       │
//! │                                                                 │
//! │ ExtensionStateBridge trait:                                     │
//! │   kind()     → &'static str (e.g., "cmdline")                   │
//! │   scope()    → ExtensionScope (Shared or Client)                │
//! │   snapshot() → Option<serde_json::Value>                        │
//! │   is_active()→ bool                                             │
//! │                                                                 │
//! │ BridgeRegistry:                                                 │
//! │   register(bridge) / get(kind) / kinds()                        │
//! │                                                                 │
//! │ BridgeProvider (Service):                                       │
//! │   Modules register bridges during init()                        │
//! │   Bootstrap collects via take_bridges()                         │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Design
//!
//! - **Mechanism**: Bridges define WHAT state to expose (driver layer)
//! - **Policy**: Modules define HOW extensions behave (module layer)
//! - **Transport**: gRPC handler calls `snapshot()` and streams JSON to clients
//! - **Scalability**: Adding plugin N+1 requires ONLY module changes

use std::{collections::HashMap, sync::Mutex};

use reovim_kernel::api::v1::{Service, ServiceRegistry};

use crate::{ClientId, ExtensionMap};

/// Scope of an extension bridge — determines which `ExtensionMap` to read from.
///
/// - `Shared`: Session-wide state (all clients see the same data)
/// - `Client`: Per-client state (each client has its own `ExtensionMap`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionScope {
    /// Session-wide state shared by all clients.
    Shared,
    /// Per-client state (each client has its own `ExtensionMap`).
    Client,
}

/// Context for cross-client reads during bridge snapshot (#543).
///
/// Constructed by the server layer inside a combined-lock closure where both
/// session locks are held. All references are valid for the closure's lifetime.
///
/// The `opponents` slice is pre-collected by the server layer, which resolves
/// `Client::effective_state()` for each connected client. This keeps the driver
/// layer free of server-layer types (`Client`, `EditingState`).
pub struct BridgeContext<'a> {
    /// The client this snapshot is being built for.
    pub client_id: ClientId,

    /// Read access to session-wide shared extensions.
    pub shared_extensions: &'a ExtensionMap,

    /// Pre-collected opponent extension maps.
    ///
    /// Each entry is `(opponent_client_id, &ExtensionMap)` for every connected
    /// client except the current one. Clients with no effective state (e.g.,
    /// broken Follow chains) are excluded by the server layer.
    opponents: &'a [(ClientId, &'a ExtensionMap)],
}

impl<'a> BridgeContext<'a> {
    /// Create a new context.
    ///
    /// Called by `Session::with_bridge_context()` in the server layer.
    #[must_use]
    pub const fn new(
        client_id: ClientId,
        shared_extensions: &'a ExtensionMap,
        opponents: &'a [(ClientId, &'a ExtensionMap)],
    ) -> Self {
        Self {
            client_id,
            shared_extensions,
            opponents,
        }
    }

    /// Iterate over all opponents' extension maps.
    ///
    /// Calls `f(client_id, extensions)` for each connected client except
    /// the current one. Skips clients with no effective state.
    pub fn for_each_opponent(&self, mut f: impl FnMut(ClientId, &ExtensionMap)) {
        for &(id, ext) in self.opponents {
            f(id, ext);
        }
    }

    /// Get the number of opponents.
    #[must_use]
    pub const fn opponent_count(&self) -> usize {
        self.opponents.len()
    }
}

/// Trait for adapting session extension state to JSON for gRPC transmission.
///
/// Each bridge maps one `SessionExtension` type to a JSON representation
/// that clients can consume. Bridges live in the driver layer (alongside
/// the state types they adapt) and are registered with the gRPC handler
/// at server startup.
///
/// # Implementors
///
/// Modules define their own bridges (e.g., `CmdlineBridge` in `reovim-module-cmdline`).
pub trait ExtensionStateBridge: Send + Sync + 'static {
    /// Unique identifier for this bridge (e.g., `"cmdline"`).
    fn kind(&self) -> &'static str;

    /// Whether this bridge reads from shared or per-client state.
    fn scope(&self) -> ExtensionScope;

    /// Serialize the extension state to JSON.
    ///
    /// Returns `None` if the extension has not been initialized in the map.
    fn snapshot(&self, extensions: &ExtensionMap) -> Option<serde_json::Value>;

    /// Check whether the extension is currently active.
    ///
    /// Used for change detection: the runner compares `is_active()` before
    /// and after key resolution to detect activation/deactivation.
    fn is_active(&self, extensions: &ExtensionMap) -> bool;

    /// Called when the client's mode changes (e.g., insert → normal).
    ///
    /// Bridges can use this to auto-deactivate when leaving a relevant mode.
    /// Default implementation is a no-op.
    ///
    /// Only called for [`ExtensionScope::Client`] bridges (#521).
    fn on_mode_changed(&self, _from: &str, _to: &str, _extensions: &mut ExtensionMap) {}

    /// Snapshot with cross-client context (multiplayer support, #543).
    ///
    /// Default delegates to [`snapshot()`](Self::snapshot) — single-player bridges
    /// need no changes. Override this to include opponent data in the snapshot.
    fn snapshot_with_context(
        &self,
        extensions: &ExtensionMap,
        _context: &BridgeContext<'_>,
    ) -> Option<serde_json::Value> {
        self.snapshot(extensions)
    }

    /// Advance extension state on a timer tick (#546).
    ///
    /// Called by the tick scheduler at regular intervals, independent of key input.
    /// Returns `true` if state was modified and clients should be notified.
    ///
    /// The `services` parameter provides access to the **live session**
    /// `ServiceRegistry`, allowing bridges to look up services (e.g.,
    /// `LspProviderRegistry`) at tick time rather than capturing `Arc`
    /// references at init time (#555).
    ///
    /// Default implementation returns `false` (no tick behavior).
    fn tick(
        &self,
        _client_extensions: &mut ExtensionMap,
        _shared_extensions: &mut ExtensionMap,
        _services: &ServiceRegistry,
    ) -> bool {
        false
    }
}

/// Registry of extension state bridges.
///
/// Holds all registered bridges and provides lookup by kind.
/// Created once at server startup and shared via `Arc<BridgeRegistry>`.
pub struct BridgeRegistry {
    bridges: HashMap<&'static str, Box<dyn ExtensionStateBridge>>,
}

impl BridgeRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bridges: HashMap::new(),
        }
    }

    /// Register a bridge. Overwrites any existing bridge with the same kind.
    pub fn register(&mut self, bridge: impl ExtensionStateBridge) {
        self.bridges.insert(bridge.kind(), Box::new(bridge));
    }

    /// Register a boxed bridge. Overwrites any existing bridge with the same kind.
    ///
    /// Used by bootstrap to move bridges from [`BridgeProvider`] into the registry.
    pub fn register_boxed(&mut self, bridge: Box<dyn ExtensionStateBridge>) {
        self.bridges.insert(bridge.kind(), bridge);
    }

    /// Look up a bridge by kind.
    #[must_use]
    pub fn get(&self, kind: &str) -> Option<&dyn ExtensionStateBridge> {
        self.bridges.get(kind).map(AsRef::as_ref)
    }

    /// List all registered bridge kinds.
    #[must_use]
    pub fn kinds(&self) -> Vec<&'static str> {
        let mut kinds: Vec<_> = self.bridges.keys().copied().collect();
        kinds.sort_unstable();
        kinds
    }

    /// Iterate over all registered bridges.
    ///
    /// Used by lifecycle hooks (e.g., `on_mode_changed`) to notify all bridges (#521).
    pub fn values(&self) -> impl Iterator<Item = &dyn ExtensionStateBridge> {
        self.bridges.values().map(AsRef::as_ref)
    }
}

impl Default for BridgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for BridgeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeRegistry")
            .field("count", &self.bridges.len())
            .field("kinds", &self.kinds())
            .finish()
    }
}

// ============================================================================
// BridgeProvider - Module-driven bridge registration
// ============================================================================

/// Service for module-driven bridge registration.
///
/// Modules call `register()` during `init()` to declare their bridges.
/// Bootstrap collects all bridges via `take_bridges()` into a [`BridgeRegistry`].
///
/// This enables plugin N+1 to register bridges without editing server/driver code.
///
/// # Example
///
/// ```ignore
/// // In module init():
/// let provider = ctx.services.get_or_create::<BridgeProvider>();
/// provider.register(MyCmdlineBridge);
///
/// // In bootstrap (after all modules init):
/// let mut registry = BridgeRegistry::new();
/// if let Some(provider) = services.get::<BridgeProvider>() {
///     for bridge in provider.take_bridges() {
///         registry.register_boxed(bridge);
///     }
/// }
/// ```
pub struct BridgeProvider {
    bridges: Mutex<Vec<Box<dyn ExtensionStateBridge>>>,
}

impl Default for BridgeProvider {
    fn default() -> Self {
        Self {
            bridges: Mutex::new(Vec::new()),
        }
    }
}

impl Service for BridgeProvider {}

impl BridgeProvider {
    /// Register a bridge for collection at startup.
    ///
    /// Called by modules during `init()`. Thread-safe via internal `Mutex`.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn register(&self, bridge: impl ExtensionStateBridge) {
        self.bridges
            .lock()
            .expect("BridgeProvider lock poisoned")
            .push(Box::new(bridge));
    }

    /// Take all registered bridges, leaving the provider empty.
    ///
    /// Called once by bootstrap to move bridges into `BridgeRegistry`.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn take_bridges(&self) -> Vec<Box<dyn ExtensionStateBridge>> {
        std::mem::take(&mut *self.bridges.lock().expect("BridgeProvider lock poisoned"))
    }
}

impl std::fmt::Debug for BridgeProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.bridges.lock().map_or(0, |b| b.len());
        f.debug_struct("BridgeProvider")
            .field("pending_bridges", &count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // ExtensionScope tests
    // ========================================================================

    #[test]
    fn test_extension_scope_debug() {
        let scope = ExtensionScope::Shared;
        let debug = format!("{scope:?}");
        assert!(debug.contains("Shared"));
    }

    #[test]
    fn test_extension_scope_clone_copy_eq() {
        let a = ExtensionScope::Client;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(a, ExtensionScope::Shared);
    }

    // ========================================================================
    // BridgeRegistry tests
    // ========================================================================

    /// Minimal bridge for testing.
    struct DummyBridge {
        kind_str: &'static str,
    }

    impl DummyBridge {
        const fn new(kind: &'static str) -> Self {
            Self { kind_str: kind }
        }
    }

    impl ExtensionStateBridge for DummyBridge {
        fn kind(&self) -> &'static str {
            self.kind_str
        }
        fn scope(&self) -> ExtensionScope {
            ExtensionScope::Shared
        }
        fn snapshot(&self, _extensions: &ExtensionMap) -> Option<serde_json::Value> {
            Some(serde_json::json!({"dummy": true}))
        }
        fn is_active(&self, _extensions: &ExtensionMap) -> bool {
            false
        }
    }

    #[test]
    fn test_registry_new_is_empty() {
        let reg = BridgeRegistry::new();
        assert!(reg.kinds().is_empty());
    }

    #[test]
    fn test_registry_default_is_empty() {
        let reg = BridgeRegistry::default();
        assert!(reg.kinds().is_empty());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut reg = BridgeRegistry::new();
        reg.register(DummyBridge::new("test"));
        assert!(reg.get("test").is_some());
        assert_eq!(reg.get("test").unwrap().kind(), "test");
    }

    #[test]
    fn test_registry_get_unknown_returns_none() {
        let reg = BridgeRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_kinds() {
        let mut reg = BridgeRegistry::new();
        reg.register(DummyBridge::new("beta"));
        reg.register(DummyBridge::new("alpha"));
        // Sorted alphabetically
        assert_eq!(reg.kinds(), vec!["alpha", "beta"]);
    }

    #[test]
    fn test_registry_duplicate_overwrites() {
        let mut reg = BridgeRegistry::new();
        reg.register(DummyBridge::new("test"));
        reg.register(DummyBridge::new("test"));
        assert_eq!(reg.kinds().len(), 1);
    }

    #[test]
    fn test_registry_debug() {
        let mut reg = BridgeRegistry::new();
        reg.register(DummyBridge::new("cmdline"));
        let debug = format!("{reg:?}");
        assert!(debug.contains("BridgeRegistry"));
        assert!(debug.contains("cmdline"));
    }

    // ========================================================================
    // Trait object construction test
    // ========================================================================

    #[test]
    fn test_trait_object_construction() {
        let bridge: Box<dyn ExtensionStateBridge> = Box::new(DummyBridge::new("test"));
        assert_eq!(bridge.kind(), "test");
        assert_eq!(bridge.scope(), ExtensionScope::Shared);
        assert!(!bridge.is_active(&ExtensionMap::new()));
    }

    #[test]
    fn test_dummy_bridge_snapshot() {
        let bridge = DummyBridge::new("test");
        let map = ExtensionMap::new();
        let snap = bridge.snapshot(&map);
        assert!(snap.is_some());
        assert_eq!(snap.unwrap(), serde_json::json!({"dummy": true}));
    }

    #[test]
    fn test_on_mode_changed_default_noop() {
        let bridge = DummyBridge::new("test");
        let mut map = ExtensionMap::new();
        // Default impl is a no-op — should not panic.
        bridge.on_mode_changed("vim:insert", "vim:normal", &mut map);
    }

    #[test]
    fn test_tick_default_returns_false() {
        let bridge = DummyBridge::new("test");
        let mut client = ExtensionMap::new();
        let mut shared = ExtensionMap::new();
        let services = ServiceRegistry::new();
        assert!(!bridge.tick(&mut client, &mut shared, &services));
    }

    #[test]
    fn test_registry_values() {
        let mut reg = BridgeRegistry::new();
        reg.register(DummyBridge::new("alpha"));
        reg.register(DummyBridge::new("beta"));
        let mut kinds: Vec<&str> = reg.values().map(ExtensionStateBridge::kind).collect();
        kinds.sort_unstable();
        assert_eq!(kinds, vec!["alpha", "beta"]);
    }

    // ========================================================================
    // register_boxed tests
    // ========================================================================

    #[test]
    fn test_registry_register_boxed() {
        let mut reg = BridgeRegistry::new();
        let boxed: Box<dyn ExtensionStateBridge> = Box::new(DummyBridge::new("boxed"));
        reg.register_boxed(boxed);
        assert!(reg.get("boxed").is_some());
        assert_eq!(reg.get("boxed").unwrap().kind(), "boxed");
    }

    // ========================================================================
    // BridgeProvider tests
    // ========================================================================

    #[test]
    fn test_bridge_provider_default_empty() {
        let provider = BridgeProvider::default();
        let bridges = provider.take_bridges();
        assert!(bridges.is_empty());
    }

    #[test]
    fn test_bridge_provider_register_and_take() {
        let provider = BridgeProvider::default();
        provider.register(DummyBridge::new("alpha"));
        provider.register(DummyBridge::new("beta"));

        let bridges = provider.take_bridges();
        assert_eq!(bridges.len(), 2);
        assert_eq!(bridges[0].kind(), "alpha");
        assert_eq!(bridges[1].kind(), "beta");

        // After take, provider is empty
        let bridges2 = provider.take_bridges();
        assert!(bridges2.is_empty());
    }

    #[test]
    fn test_bridge_provider_service_trait() {
        use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

        let registry = ServiceRegistry::new();
        let provider = registry.get_or_create::<BridgeProvider>();
        provider.register(DummyBridge::new("test"));

        // Retrieve same instance
        let provider2 = registry.get_or_create::<BridgeProvider>();
        assert_eq!(Arc::as_ptr(&provider), Arc::as_ptr(&provider2));

        let bridges = provider2.take_bridges();
        assert_eq!(bridges.len(), 1);
    }

    #[test]
    fn test_bridge_provider_debug() {
        let provider = BridgeProvider::default();
        provider.register(DummyBridge::new("test"));
        let debug = format!("{provider:?}");
        assert!(debug.contains("BridgeProvider"));
        assert!(debug.contains("pending_bridges"));
    }

    // ========================================================================
    // BridgeContext tests (#543)
    // ========================================================================

    #[test]
    fn test_bridge_context_new() {
        let shared = ExtensionMap::new();
        let opp1 = ExtensionMap::new();
        let opponents = vec![(ClientId::new(2), &opp1)];
        let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

        assert_eq!(ctx.client_id, ClientId::new(1));
        assert_eq!(ctx.opponent_count(), 1);
    }

    #[test]
    fn test_bridge_context_for_each_opponent_iterates_all() {
        let shared = ExtensionMap::new();
        let opp1 = ExtensionMap::new();
        let opp2 = ExtensionMap::new();
        let opponents = vec![(ClientId::new(2), &opp1), (ClientId::new(3), &opp2)];
        let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

        let mut ids = Vec::new();
        ctx.for_each_opponent(|id, _ext| ids.push(id));
        assert_eq!(ids, vec![ClientId::new(2), ClientId::new(3)]);
    }

    #[test]
    fn test_bridge_context_for_each_opponent_empty() {
        let shared = ExtensionMap::new();
        let opponents: Vec<(ClientId, &ExtensionMap)> = vec![];
        let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

        let mut count = 0;
        ctx.for_each_opponent(|_, _| count += 1);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_bridge_context_opponent_count() {
        let shared = ExtensionMap::new();
        let opp1 = ExtensionMap::new();
        let opponents = vec![(ClientId::new(5), &opp1)];
        let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);
        assert_eq!(ctx.opponent_count(), 1);

        let empty: Vec<(ClientId, &ExtensionMap)> = vec![];
        let ctx2 = BridgeContext::new(ClientId::new(1), &shared, &empty);
        assert_eq!(ctx2.opponent_count(), 0);
    }

    #[test]
    fn test_bridge_context_shared_extensions_access() {
        use crate::SessionExtension;

        #[derive(Debug)]
        struct SharedExt {
            value: i32,
        }
        impl SessionExtension for SharedExt {
            fn create() -> Self {
                Self { value: 42 }
            }
        }

        let mut shared = ExtensionMap::new();
        shared.get_or_insert::<SharedExt>();
        let opponents: Vec<(ClientId, &ExtensionMap)> = vec![];
        let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

        let ext = ctx.shared_extensions.get::<SharedExt>();
        assert!(ext.is_some());
        assert_eq!(ext.unwrap().value, 42);
    }

    // ========================================================================
    // snapshot_with_context tests (#543)
    // ========================================================================

    #[test]
    fn test_snapshot_with_context_default_delegates() {
        let bridge = DummyBridge::new("test");
        let map = ExtensionMap::new();
        let shared = ExtensionMap::new();
        let opponents: Vec<(ClientId, &ExtensionMap)> = vec![];
        let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

        // Default snapshot_with_context delegates to snapshot
        let snap = bridge.snapshot_with_context(&map, &ctx);
        let direct = bridge.snapshot(&map);
        assert_eq!(snap, direct);
    }

    #[test]
    fn test_snapshot_with_context_override_uses_context() {
        use crate::SessionExtension;

        #[derive(Debug)]
        struct OpponentScore {
            score: u32,
        }
        impl SessionExtension for OpponentScore {
            fn create() -> Self {
                Self { score: 0 }
            }
        }

        /// Bridge that reads opponent data from context.
        struct MultiplayerBridge;

        impl ExtensionStateBridge for MultiplayerBridge {
            fn kind(&self) -> &'static str {
                "multiplayer"
            }
            fn scope(&self) -> ExtensionScope {
                ExtensionScope::Client
            }
            fn snapshot(&self, _ext: &ExtensionMap) -> Option<serde_json::Value> {
                Some(serde_json::json!({"score": 0}))
            }
            fn is_active(&self, _ext: &ExtensionMap) -> bool {
                true
            }
            fn snapshot_with_context(
                &self,
                _ext: &ExtensionMap,
                context: &BridgeContext<'_>,
            ) -> Option<serde_json::Value> {
                let mut opponent_scores = Vec::new();
                context.for_each_opponent(|id, ext| {
                    if let Some(score) = ext.get::<OpponentScore>() {
                        opponent_scores
                            .push(serde_json::json!({"id": id.as_usize(), "score": score.score}));
                    }
                });
                Some(serde_json::json!({"opponents": opponent_scores}))
            }
        }

        // Setup: one opponent with a score
        let mut opp_map = ExtensionMap::new();
        let opp_score = opp_map.get_or_insert::<OpponentScore>();
        opp_score.score = 999;

        let shared = ExtensionMap::new();
        let opponents = vec![(ClientId::new(2), &opp_map)];
        let ctx = BridgeContext::new(ClientId::new(1), &shared, &opponents);

        let bridge = MultiplayerBridge;
        let own_map = ExtensionMap::new();
        let snap = bridge.snapshot_with_context(&own_map, &ctx);

        assert!(snap.is_some());
        let json = snap.unwrap();
        let opponents_arr = json["opponents"].as_array().unwrap();
        assert_eq!(opponents_arr.len(), 1);
        assert_eq!(opponents_arr[0]["id"], 2);
        assert_eq!(opponents_arr[0]["score"], 999);
    }
}
