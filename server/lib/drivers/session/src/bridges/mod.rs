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
    /// Extension kinds declared by loaded server modules (#584).
    available_kinds: Vec<&'static str>,
}

impl BridgeRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bridges: HashMap::new(),
            available_kinds: Vec::new(),
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

    /// Set the extension kinds declared by loaded server modules (#584).
    pub fn set_available_kinds(&mut self, kinds: Vec<&'static str>) {
        self.available_kinds = kinds;
    }

    /// Extension kinds declared by loaded server modules (#584).
    ///
    /// Used by gRPC service to expose available kinds to clients.
    #[must_use]
    pub fn available_kinds(&self) -> &[&'static str] {
        &self.available_kinds
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
            .field("available_kinds", &self.available_kinds)
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
#[path = "tests/mod_tests.rs"]
mod tests;
