//! Plugin context for component registration

use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    bind::{CommandRef, KeyMap, KeymapScope},
    command::{CommandRegistry, CommandTrait},
    display::{DisplayInfo, DisplayRegistry, EditModeKey, SubModeKey},
    interactor::InteractorRegistry,
    keystroke::KeySequence,
    modifier::ModifierRegistry,
    overlay::{OverlayRegistry, OverlayRenderer},
    rpc::{RpcHandler, RpcHandlerRegistry},
    runtime::FocusInputHandler,
    ui_component::{ComponentId, ComponentRegistry, UIComponent},
};

/// Context passed to plugins during the build phase
///
/// This provides access to all registries and allows plugins
/// to register their components in a structured way.
pub struct PluginContext {
    /// Command registry for registering commands
    pub(crate) commands: CommandRegistry,

    /// Interactor registry for UI component handlers
    pub(crate) interactors: InteractorRegistry,

    /// Modifier registry for style/behavior modifiers
    pub(crate) modifiers: ModifierRegistry,

    /// Keymap for keybinding registration
    pub(crate) keymap: KeyMap,

    /// Focus input handlers (enlist pattern)
    pub(crate) focus_handlers: HashMap<ComponentId, FocusInputHandler>,

    /// Track which plugins have been loaded
    pub(crate) loaded_plugins: std::collections::HashSet<TypeId>,

    /// Component registry for UI components
    pub(crate) components: ComponentRegistry,

    /// Overlay registry for plugin-based overlays
    pub(crate) overlays: OverlayRegistry,

    /// RPC handler registry for plugin-registered RPC methods
    pub(crate) rpc_handlers: RpcHandlerRegistry,

    /// Display registry for plugin-provided mode display strings and icons
    pub(crate) display_registry: DisplayRegistry,
}

impl Default for PluginContext {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginContext {
    /// Create a new empty plugin context
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: CommandRegistry::new(),
            interactors: InteractorRegistry::new(),
            modifiers: ModifierRegistry::new(),
            keymap: KeyMap::default(),
            focus_handlers: HashMap::new(),
            loaded_plugins: std::collections::HashSet::new(),
            components: ComponentRegistry::new(),
            overlays: OverlayRegistry::new(),
            rpc_handlers: RpcHandlerRegistry::new(),
            display_registry: DisplayRegistry::new(),
        }
    }

    // === Command Registration ===

    /// Register a command
    ///
    /// # Errors
    /// Returns error if command with same name is already registered.
    pub fn register_command<C: CommandTrait + 'static>(
        &self,
        cmd: C,
    ) -> Result<(), crate::command::RegistryError> {
        self.commands.register(cmd)
    }

    /// Register a command, replacing any existing command with the same ID
    pub fn register_command_or_replace<C: CommandTrait + 'static>(&self, cmd: C) {
        self.commands.register_or_replace(cmd);
    }

    /// Get access to the command registry
    #[must_use]
    pub const fn command_registry(&self) -> &CommandRegistry {
        &self.commands
    }

    // === Interactor Registration ===

    /// Register an interactor (UI component)
    pub fn register_interactor(&mut self, interactor: Box<dyn UIComponent>) {
        self.interactors.register(interactor);
    }

    /// Register a focus input handler for an interactor
    ///
    /// This uses the "enlist" pattern where handlers are function pointers
    /// that receive the runtime and input data.
    pub fn register_focus_handler(&mut self, id: ComponentId, handler: FocusInputHandler) {
        self.focus_handlers.insert(id, handler);
    }

    /// Get access to the interactor registry
    #[must_use]
    pub const fn interactor_registry(&self) -> &InteractorRegistry {
        &self.interactors
    }

    /// Get mutable access to the interactor registry
    #[must_use]
    pub const fn interactor_registry_mut(&mut self) -> &mut InteractorRegistry {
        &mut self.interactors
    }

    // === Keybinding Registration ===

    /// Get access to the keymap
    #[must_use]
    pub const fn keymap(&self) -> &KeyMap {
        &self.keymap
    }

    /// Get mutable access to the keymap
    #[must_use]
    pub const fn keymap_mut(&mut self) -> &mut KeyMap {
        &mut self.keymap
    }

    // === Modifier Registration ===

    /// Get access to the modifier registry
    #[must_use]
    pub const fn modifier_registry(&self) -> &ModifierRegistry {
        &self.modifiers
    }

    /// Get mutable access to the modifier registry
    #[must_use]
    pub const fn modifier_registry_mut(&mut self) -> &mut ModifierRegistry {
        &mut self.modifiers
    }

    // === Component Registration ===

    /// Register a UI component
    ///
    /// If a component with the same ID already exists, it will be replaced.
    /// This also automatically registers any keybindings provided by the component
    /// via `UIComponent::keybindings()`.
    pub fn register_component(&mut self, component: Box<dyn UIComponent>) {
        let id = component.id();

        // Register keybindings from the component
        for (mode, binding) in component.keybindings() {
            let scope = KeymapScope::Component { id, mode };
            self.keymap.bind_with_metadata(scope, binding);
        }

        // Register the component itself
        self.components.register(component);
    }

    /// Bind a key sequence to a command with a specific scope
    ///
    /// This allows direct scope-based keybinding registration without
    /// going through the legacy mode string API.
    pub fn bind_key_scoped(&mut self, scope: KeymapScope, keys: KeySequence, cmd: CommandRef) {
        self.keymap.bind_scoped(scope, keys, cmd);
    }

    /// Get access to the component registry
    #[must_use]
    pub const fn component_registry(&self) -> &ComponentRegistry {
        &self.components
    }

    /// Get mutable access to the component registry
    #[must_use]
    pub const fn component_registry_mut(&mut self) -> &mut ComponentRegistry {
        &mut self.components
    }

    // === Overlay Registration ===

    /// Register an overlay renderer
    ///
    /// Overlays are rendered in z-order during screen updates.
    /// Lower z-order overlays are drawn first (behind higher ones).
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.register_overlay(MyOverlay::new());
    /// ```
    pub fn register_overlay<O: OverlayRenderer + 'static>(&mut self, overlay: O) {
        self.overlays.register(overlay);
    }

    /// Register an overlay from an Arc (for shared overlays)
    pub fn register_overlay_arc(&mut self, overlay: Arc<RwLock<dyn OverlayRenderer>>) {
        self.overlays.register_arc(overlay);
    }

    /// Get access to the overlay registry
    #[must_use]
    pub const fn overlay_registry(&self) -> &OverlayRegistry {
        &self.overlays
    }

    /// Get mutable access to the overlay registry
    #[must_use]
    pub const fn overlay_registry_mut(&mut self) -> &mut OverlayRegistry {
        &mut self.overlays
    }

    // === RPC Handler Registration ===

    /// Register an RPC handler
    ///
    /// RPC handlers respond to JSON-RPC method calls when the editor
    /// is running in server mode.
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.register_rpc_handler(Arc::new(MyRpcHandler));
    /// ```
    pub fn register_rpc_handler(&mut self, handler: Arc<dyn RpcHandler>) {
        self.rpc_handlers.register(handler);
    }

    /// Get access to the RPC handler registry
    #[must_use]
    pub const fn rpc_handler_registry(&self) -> &RpcHandlerRegistry {
        &self.rpc_handlers
    }

    /// Get mutable access to the RPC handler registry
    #[must_use]
    pub const fn rpc_handler_registry_mut(&mut self) -> &mut RpcHandlerRegistry {
        &mut self.rpc_handlers
    }

    // === Display Registration ===

    /// Register display info for a component focus
    ///
    /// This sets the display string and icon shown when the component is focused.
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.register_display(ComponentId("explorer"), DisplayInfo::new(" EXPLORER ", "󰙅 "));
    /// ```
    pub fn register_display(&mut self, id: ComponentId, info: DisplayInfo) {
        self.display_registry.register_interactor(id, info);
    }

    /// Register display info for a component + edit mode combination
    ///
    /// This allows different display strings for the same component
    /// depending on the edit mode (Normal, Insert, Visual, etc.).
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.register_component_mode_display(
    ///     ComponentId::EDITOR,
    ///     EditModeKey::Normal,
    ///     DisplayInfo::new(" NORMAL ", "󰆾 "),
    /// );
    /// ```
    pub fn register_component_mode_display(
        &mut self,
        id: ComponentId,
        mode: EditModeKey,
        info: DisplayInfo,
    ) {
        self.display_registry
            .register_component_mode(id, mode, info);
    }

    /// Register display info for a sub-mode
    ///
    /// Sub-modes (Command, `OperatorPending`, Leap, etc.) take priority
    /// over component/edit mode displays.
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.register_sub_mode_display(SubModeKey::Command, DisplayInfo::new(" COMMAND ", " "));
    /// ```
    pub fn register_sub_mode_display(&mut self, key: SubModeKey, info: DisplayInfo) {
        self.display_registry.register_sub_mode(key, info);
    }

    /// Get access to the display registry
    #[must_use]
    pub const fn display_registry(&self) -> &DisplayRegistry {
        &self.display_registry
    }

    /// Get mutable access to the display registry
    #[must_use]
    pub const fn display_registry_mut(&mut self) -> &mut DisplayRegistry {
        &mut self.display_registry
    }

    // === Plugin Queries ===

    /// Check if a plugin has been loaded
    #[must_use]
    pub fn has_plugin<P: 'static>(&self) -> bool {
        self.loaded_plugins.contains(&TypeId::of::<P>())
    }

    /// Mark a plugin as loaded (internal use)
    #[allow(dead_code)]
    pub(crate) fn mark_loaded<P: 'static>(&mut self) {
        self.loaded_plugins.insert(TypeId::of::<P>());
    }

    /// Mark a plugin as loaded by type ID (internal use)
    pub(crate) fn mark_loaded_by_id(&mut self, type_id: TypeId) {
        self.loaded_plugins.insert(type_id);
    }

    // === Consume Context ===

    /// Consume the context and return all components
    ///
    /// This is used by Runtime to take ownership of all registered components.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        CommandRegistry,
        InteractorRegistry,
        ModifierRegistry,
        KeyMap,
        HashMap<ComponentId, FocusInputHandler>,
        ComponentRegistry,
        OverlayRegistry,
        RpcHandlerRegistry,
        DisplayRegistry,
    ) {
        (
            self.commands,
            self.interactors,
            self.modifiers,
            self.keymap,
            self.focus_handlers,
            self.components,
            self.overlays,
            self.rpc_handlers,
            self.display_registry,
        )
    }
}
