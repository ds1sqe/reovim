//! Extension state bridge system for server-to-client state streaming.
//!
//! This module provides the [`ExtensionStateBridge`] trait that adapts session
//! extension state (e.g., `CmdlineState`) into JSON for gRPC transmission to
//! clients. The runner registers bridges with the gRPC handler at startup.
//!
//! # Architecture (#514)
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │ SESSION DRIVER (bridges/)                                       │
//! │                                                                 │
//! │ ExtensionStateBridge trait:                                     │
//! │   kind()     → &'static str (e.g., "cmdline")                  │
//! │   scope()    → ExtensionScope (Shared or Client)                │
//! │   snapshot() → Option<serde_json::Value>                        │
//! │   is_active()→ bool                                             │
//! │                                                                 │
//! │ BridgeRegistry:                                                 │
//! │   register(bridge) / get(kind) / kinds()                        │
//! │                                                                 │
//! │ Concrete bridges:                                               │
//! │   CmdlineBridge → CmdlineState                                  │
//! └──────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Design
//!
//! - **Mechanism**: Bridges define WHAT state to expose (driver layer)
//! - **Policy**: Modules define HOW extensions behave (module layer)
//! - **Transport**: gRPC handler calls `snapshot()` and streams JSON to clients

mod cmdline;

pub use cmdline::CmdlineBridge;

use std::collections::HashMap;

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
/// - [`CmdlineBridge`] — adapts `CmdlineState` (per-client)
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
}
