//! Plugin context for component registration

use std::{any::TypeId, borrow::Cow, sync::Arc};

use crate::{
    bind::{CommandRef, KeyMap, KeymapScope},
    command::{CommandRegistry, CommandTrait},
    display::{DisplayInfo, DisplayRegistry, EditModeKey, SubModeKey},
    keystroke::KeySequence,
    modd::ComponentId,
    modifier::ModifierRegistry,
    option::{OptionCategory, OptionConstraint, OptionError, OptionScope, OptionSpec, OptionValue},
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

    /// Pending option specifications to be registered
    pub(crate) option_specs: Vec<OptionSpec>,

    /// Current plugin ID for namespacing options
    pub(crate) current_plugin_id: Option<String>,
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
            keymap: KeyMap::with_defaults(),
            loaded_plugins: std::collections::HashSet::new(),
            rpc_handlers: RpcHandlerRegistry::new(),
            display_registry: DisplayRegistry::new(),
            render_stages: RenderStageRegistry::new(),
            option_specs: Vec::new(),
            current_plugin_id: None,
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

    // === Option Registration ===

    /// Set the current plugin ID for option namespacing
    ///
    /// This is called by the plugin loader before building each plugin.
    /// Options registered via `option()` will be namespaced under this ID.
    pub fn set_current_plugin(&mut self, plugin_id: impl Into<String>) {
        self.current_plugin_id = Some(plugin_id.into());
    }

    /// Clear the current plugin ID
    pub fn clear_current_plugin(&mut self) {
        self.current_plugin_id = None;
    }

    /// Create an option builder for registering a plugin option
    ///
    /// Options are automatically namespaced under `plugin.{plugin_id}.{name}`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl Plugin for MyPlugin {
    ///     fn build(&self, ctx: &mut PluginContext) {
    ///         ctx.option("highlight_timeout_ms")
    ///             .description("Timeout for highlighting in milliseconds")
    ///             .default_int(100)
    ///             .min(10)
    ///             .max(1000)
    ///             .register()?;
    ///     }
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if called outside of plugin build phase (no current plugin ID set).
    #[must_use]
    pub fn option(&mut self, name: &'static str) -> PluginOptionBuilder<'_> {
        let plugin_id = self
            .current_plugin_id
            .clone()
            .expect("option() called outside plugin build phase");
        PluginOptionBuilder::new(self, name, plugin_id)
    }

    /// Get the pending option specifications
    #[must_use]
    pub fn option_specs(&self) -> &[OptionSpec] {
        &self.option_specs
    }

    /// Take the pending option specifications (consumes them)
    #[must_use]
    pub fn take_option_specs(&mut self) -> Vec<OptionSpec> {
        std::mem::take(&mut self.option_specs)
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
        Vec<OptionSpec>,
    ) {
        (
            self.commands,
            self.modifiers,
            self.keymap,
            self.rpc_handlers,
            self.display_registry,
            self.render_stages,
            self.option_specs,
        )
    }
}

/// Builder for registering plugin options with a fluent API.
///
/// Created via [`PluginContext::option()`].
pub struct PluginOptionBuilder<'a> {
    ctx: &'a mut PluginContext,
    name: &'static str,
    plugin_id: String,
    short: Option<Cow<'static, str>>,
    description: Cow<'static, str>,
    default: OptionValue,
    constraint: OptionConstraint,
    scope: OptionScope,
}

impl<'a> PluginOptionBuilder<'a> {
    const fn new(ctx: &'a mut PluginContext, name: &'static str, plugin_id: String) -> Self {
        Self {
            ctx,
            name,
            plugin_id,
            short: None,
            description: Cow::Borrowed(""),
            default: OptionValue::Bool(false),
            constraint: OptionConstraint::none(),
            scope: OptionScope::Global,
        }
    }

    /// Set a short alias for the option.
    #[must_use]
    pub fn short(mut self, alias: impl Into<Cow<'static, str>>) -> Self {
        self.short = Some(alias.into());
        self
    }

    /// Set the description for the option.
    #[must_use]
    pub fn description(mut self, desc: impl Into<Cow<'static, str>>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the default value to a boolean.
    #[must_use]
    pub fn default_bool(mut self, value: bool) -> Self {
        self.default = OptionValue::Bool(value);
        self
    }

    /// Set the default value to an integer.
    #[must_use]
    pub fn default_int(mut self, value: i64) -> Self {
        self.default = OptionValue::Integer(value);
        self
    }

    /// Set the default value to a string.
    #[must_use]
    pub fn default_string(mut self, value: impl Into<String>) -> Self {
        self.default = OptionValue::String(value.into());
        self
    }

    /// Set the default value to a choice from predefined values.
    #[must_use]
    pub fn default_choice(mut self, value: impl Into<String>, choices: &[&str]) -> Self {
        self.default = OptionValue::Choice {
            value: value.into(),
            choices: choices.iter().map(|s| (*s).to_string()).collect(),
        };
        self
    }

    /// Set a minimum value constraint (for integer options).
    #[must_use]
    pub const fn min(mut self, value: i64) -> Self {
        self.constraint.min = Some(value);
        self
    }

    /// Set a maximum value constraint (for integer options).
    #[must_use]
    pub const fn max(mut self, value: i64) -> Self {
        self.constraint.max = Some(value);
        self
    }

    /// Set both min and max constraints (for integer options).
    #[must_use]
    pub const fn range(mut self, min: i64, max: i64) -> Self {
        self.constraint.min = Some(min);
        self.constraint.max = Some(max);
        self
    }

    /// Set the scope for this option.
    #[must_use]
    pub const fn scope(mut self, scope: OptionScope) -> Self {
        self.scope = scope;
        self
    }

    /// Register the option.
    ///
    /// # Errors
    ///
    /// Returns error if an option with the same name already exists.
    pub fn register(self) -> Result<(), OptionError> {
        let full_name: Cow<'static, str> =
            format!("plugin.{}.{}", self.plugin_id, self.name).into();

        let spec = OptionSpec {
            name: full_name,
            short: self.short,
            description: self.description,
            category: OptionCategory::Plugin(self.plugin_id.into()),
            default: self.default,
            constraint: self.constraint,
            depends_on: Vec::new(),
            scope: self.scope,
            section: None,
            display_order: 100,
            show_in_menu: true,
        };

        // Check for duplicates
        if self.ctx.option_specs.iter().any(|s| s.name == spec.name) {
            return Err(OptionError::AlreadyExists(spec.name.to_string()));
        }

        self.ctx.option_specs.push(spec);
        Ok(())
    }
}
