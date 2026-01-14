//! Core plugin traits and types

use std::{any::TypeId, sync::Arc};

use tokio::sync::mpsc;

use crate::{event::RuntimeEvent, event_bus::EventBus};

use super::{PluginContext, PluginStateRegistry};

/// Unique identifier for a plugin
///
/// Plugin IDs use a namespace:name format, e.g., "reovim:core" or "community:git".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(&'static str);

impl PluginId {
    /// Create a new plugin ID
    ///
    /// Convention: Use "namespace:name" format
    /// - "reovim:*" for built-in plugins
    /// - "community:*" for community plugins
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Get the plugin ID as a string slice
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Trait for plugins that provide modular functionality
///
/// Plugins encapsulate initialization logic and can declare dependencies
/// on other plugins. The plugin system resolves dependencies and ensures
/// plugins are initialized in the correct order.
///
/// # Example
///
/// ```ignore
/// pub struct MyPlugin;
///
/// impl Plugin for MyPlugin {
///     fn id(&self) -> PluginId {
///         PluginId::new("my:plugin")
///     }
///
///     fn build(&self, ctx: &mut PluginContext) {
///         // Register commands, interactors, keybindings, etc.
///         ctx.register_command(MyCommand);
///     }
///
///     fn dependencies(&self) -> Vec<TypeId> {
///         // Declare that this plugin requires CorePlugin
///         vec![TypeId::of::<CorePlugin>()]
///     }
/// }
/// ```
pub trait Plugin: Send + Sync + 'static {
    /// Unique identifier for this plugin
    fn id(&self) -> PluginId;

    /// Human-readable name for this plugin
    ///
    /// Defaults to the type name.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// Description of what this plugin provides
    fn description(&self) -> &'static str {
        ""
    }

    /// Build phase: Register components with the plugin context
    ///
    /// This is where plugins register their commands, interactors,
    /// keybindings, pickers, and handlers.
    fn build(&self, ctx: &mut PluginContext);

    /// Finalize phase after all plugins are built
    ///
    /// Called after all plugins have been built, useful for
    /// initialization that depends on other plugins being present.
    fn finish(&self, _ctx: &mut PluginContext) {}

    /// Declare plugin dependencies
    ///
    /// Returns a list of `TypeId`s for plugins this plugin depends on.
    /// The plugin system ensures dependencies are loaded first.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn dependencies(&self) -> Vec<TypeId> {
    ///     vec![TypeId::of::<CorePlugin>()]
    /// }
    /// ```
    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    /// Optional plugins that should be loaded before this one if present
    ///
    /// Unlike dependencies, these are not required to be present.
    fn optional_dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    // =========================================================================
    // NEW: Runtime Lifecycle Methods
    // =========================================================================

    /// Initialize plugin state
    ///
    /// Called after all plugins are built, before the event loop starts.
    /// Plugins should register their state with the registry here.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn init_state(&self, registry: &PluginStateRegistry) {
    ///     registry.register(MyPluginState::new());
    /// }
    /// ```
    fn init_state(&self, _registry: &PluginStateRegistry) {
        // Default: no state to initialize
    }

    /// Subscribe to events via the event bus
    ///
    /// Called after all plugins have initialized their state.
    /// Plugins should register event handlers here.
    ///
    /// # Arguments
    ///
    /// * `bus` - The event bus to subscribe to
    /// * `state` - Shared reference to the plugin state registry
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
    ///     let state_clone = state.clone();
    ///     bus.subscribe::<MyEvent, _>(100, move |event, ctx| {
    ///         state_clone.with_mut::<MyPluginState, _, _>(|s| {
    ///             s.handle_event(event);
    ///         });
    ///         EventResult::Handled
    ///     });
    /// }
    /// ```
    fn subscribe(&self, _bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Default: no event subscriptions
    }

    /// Boot phase: Called after `EventBus` is active and initial events processed
    ///
    /// This is the final initialization phase, guaranteed to run after:
    /// - All plugins have subscribed to events
    /// - `EventBus` processor is running
    /// - Initial events (like `RegisterLanguage`) have been dispatched
    ///
    /// Use this for initialization that requires the event bus to be live
    /// and all plugins to be fully initialized.
    ///
    /// # Arguments
    ///
    /// * `bus` - The event bus (now active and processing events)
    /// * `state` - Shared reference to the plugin state registry
    /// * `event_tx` - Optional sender for `RuntimeEvent` (for spawning background tasks
    ///   that need to signal re-renders). Plugins that don't need this can ignore it.
    fn boot(
        &self,
        _bus: &EventBus,
        _state: Arc<PluginStateRegistry>,
        _event_tx: Option<mpsc::Sender<RuntimeEvent>>,
    ) {
        // Default: no boot-time initialization
    }
}
