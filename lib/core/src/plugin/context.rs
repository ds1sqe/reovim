//! Plugin context for component registration

use std::{any::TypeId, collections::HashMap, sync::Arc};

use crate::{
    bind::{CommandRef, KeyMap},
    command::{CommandId, CommandRegistry, CommandTrait},
    interactor::{Interactor, InteractorId, InteractorRegistry},
    modifier::ModifierRegistry,
    runtime::FocusInputHandler,
    telescope::picker::Picker,
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

    /// Telescope pickers
    pub(crate) pickers: HashMap<String, Arc<dyn Picker>>,

    /// Focus input handlers (enlist pattern)
    pub(crate) focus_handlers: HashMap<InteractorId, FocusInputHandler>,

    /// Track which plugins have been loaded
    pub(crate) loaded_plugins: std::collections::HashSet<TypeId>,
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
            pickers: HashMap::new(),
            focus_handlers: HashMap::new(),
            loaded_plugins: std::collections::HashSet::new(),
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

    /// Register an interactor
    pub fn register_interactor(&mut self, interactor: Box<dyn Interactor>) {
        self.interactors.register(interactor);
    }

    /// Register a focus input handler for an interactor
    ///
    /// This uses the "enlist" pattern where handlers are function pointers
    /// that receive the runtime and input data.
    pub fn register_focus_handler(&mut self, id: InteractorId, handler: FocusInputHandler) {
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

    /// Bind a key sequence to a command reference
    ///
    /// # Arguments
    /// * `mode` - The mode name (e.g., "normal", "insert", "visual")
    /// * `keys` - The key sequence (e.g., "j", " ff", "gg")
    /// * `cmd` - The command reference to bind
    pub fn bind_key(&mut self, mode: &str, keys: &str, cmd: CommandRef) {
        self.keymap.bind(mode, keys, cmd);
    }

    /// Bind a key sequence to a command ID
    pub fn bind_key_to_id(&mut self, mode: &str, keys: &str, id: CommandId) {
        self.keymap.bind(mode, keys, CommandRef::Registered(id));
    }

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

    // === Picker Registration ===

    /// Register a telescope picker
    pub fn register_picker<P: Picker + 'static>(&mut self, picker: P) {
        self.pickers
            .insert(picker.name().to_string(), Arc::new(picker));
    }

    /// Get a picker by name
    #[must_use]
    pub fn get_picker(&self, name: &str) -> Option<Arc<dyn Picker>> {
        self.pickers.get(name).cloned()
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
        HashMap<String, Arc<dyn Picker>>,
        HashMap<InteractorId, FocusInputHandler>,
    ) {
        (
            self.commands,
            self.interactors,
            self.modifiers,
            self.keymap,
            self.pickers,
            self.focus_handlers,
        )
    }
}
