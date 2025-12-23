//! Plugin context for component registration

use std::{any::TypeId, sync::Arc};

use crate::{
    bind::{CommandRef, KeyMap, KeymapScope},
    command::{CommandRegistry, CommandTrait},
    display::{DisplayInfo, DisplayRegistry, EditModeKey, SubModeKey},
    keystroke::KeySequence,
    modd::ComponentId,
    modifier::ModifierRegistry,
    render::{RenderStage, RenderStageRegistry},
    rpc::{RpcHandler, RpcHandlerRegistry},
};

/// Context passed to plugins during the build phase
///
/// This provides access to all registries and allows plugins
/// to register their components in a structured way.
pub struct PluginContext {
    /// Command registry for registering commands
    pub(crate) commands: CommandRegistry,

    /// Modifier registry for style/behavior modifiers
    pub(crate) modifiers: ModifierRegistry,

    /// Keymap for keybinding registration
    pub(crate) keymap: KeyMap,

    /// Track which plugins have been loaded
    pub(crate) loaded_plugins: std::collections::HashSet<TypeId>,

    /// RPC handler registry for plugin-registered RPC methods
    pub(crate) rpc_handlers: RpcHandlerRegistry,

    /// Display registry for plugin-provided mode display strings and icons
    pub(crate) display_registry: DisplayRegistry,

    /// Render stage registry for pipeline stages
    pub(crate) render_stages: RenderStageRegistry,
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
            modifiers: ModifierRegistry::new(),
            keymap: KeyMap::default(),
            loaded_plugins: std::collections::HashSet::new(),
            rpc_handlers: RpcHandlerRegistry::new(),
            display_registry: DisplayRegistry::new(),
            render_stages: RenderStageRegistry::new(),
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

    /// Bind a key sequence to a command with a specific scope
    ///
    /// This allows direct scope-based keybinding registration without
    /// going through the legacy mode string API.
    pub fn bind_key_scoped(&mut self, scope: KeymapScope, keys: KeySequence, cmd: CommandRef) {
        self.keymap.bind_scoped(scope, keys, cmd);
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

    /// Create a display info builder for fluent registration
    ///
    /// Provides a builder pattern for registering display info across multiple scopes.
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.display_info(ComponentId("explorer"))
    ///     .default(" EXPLORER ", "󰙅 ")
    ///     .when_mode(&EditMode::Insert(InsertVariant::Standard), " EXPLORER | INSERT ", "󰙅 ")
    ///     .register();
    /// ```
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Cannot be const - returns builder with mutable borrow
    pub fn display_info(&mut self, id: ComponentId) -> crate::display::DisplayInfoBuilder<'_> {
        crate::display::DisplayInfoBuilder::new(self, id)
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

    // === Render Stage Registration ===

    /// Register a render pipeline stage
    ///
    /// Render stages transform buffer content during rendering.
    /// They are executed in registration order:
    /// 1. Visibility (folding)
    /// 2. Syntax highlighting
    /// 3. Decorations (markdown conceals, etc.)
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.register_render_stage(Arc::new(MyRenderStage::new()));
    /// ```
    pub fn register_render_stage(&mut self, stage: Arc<dyn RenderStage>) {
        self.render_stages.register(stage);
    }

    /// Get access to the render stage registry
    #[must_use]
    pub const fn render_stage_registry(&self) -> &RenderStageRegistry {
        &self.render_stages
    }

    /// Get mutable access to the render stage registry
    #[must_use]
    pub const fn render_stage_registry_mut(&mut self) -> &mut RenderStageRegistry {
        &mut self.render_stages
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
        ModifierRegistry,
        KeyMap,
        RpcHandlerRegistry,
        DisplayRegistry,
        RenderStageRegistry,
    ) {
        (
            self.commands,
            self.modifiers,
            self.keymap,
            self.rpc_handlers,
            self.display_registry,
            self.render_stages,
        )
    }
}
