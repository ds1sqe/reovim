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

use reovim_kernel::api::v1::Service;

use crate::ExtensionMap;

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
}
