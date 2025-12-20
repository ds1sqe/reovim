//! Plugin context for component registration

use std::{any::TypeId, collections::HashMap, sync::Arc};

use crate::{
    bind::{CommandRef, KeyMap, KeymapScope},
    command::{CommandRegistry, CommandTrait},
    interactor::InteractorRegistry,
    keystroke::KeySequence,
    modifier::ModifierRegistry,
    runtime::FocusInputHandler,
    telescope::picker::Picker,
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

    /// Telescope pickers
    pub(crate) pickers: HashMap<String, Arc<dyn Picker>>,

    /// Focus input handlers (enlist pattern)
    pub(crate) focus_handlers: HashMap<ComponentId, FocusInputHandler>,

    /// Track which plugins have been loaded
    pub(crate) loaded_plugins: std::collections::HashSet<TypeId>,

    /// Component registry for UI components
    pub(crate) components: ComponentRegistry,
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
            components: ComponentRegistry::new(),
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
        HashMap<ComponentId, FocusInputHandler>,
        ComponentRegistry,
    ) {
        (
            self.commands,
            self.interactors,
            self.modifiers,
            self.keymap,
            self.pickers,
            self.focus_handlers,
            self.components,
        )
    }
}
