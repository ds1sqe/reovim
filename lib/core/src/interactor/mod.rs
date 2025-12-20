//! Extensible interactor system for UI components that handle user input
//!
//! The interactor system uses a trait-based approach allowing plugins to register
//! custom interactors without modifying core code.
//!
//! Conceptual model:
//! ```text
//! User <-> Window <- Interactor -> Buffer
//!          (view)    (input handler)  (data)
//! ```

use std::collections::HashMap;

use crate::{event::InnerEvent, modd::ModeState};

/// Unique identifier for an interactor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InteractorId(pub &'static str);

impl InteractorId {
    /// Editor interactor (main text editing area)
    pub const EDITOR: Self = Self("editor");
    /// Explorer interactor (file browser sidebar)
    pub const EXPLORER: Self = Self("explorer");
    /// Telescope interactor (fuzzy finder)
    pub const TELESCOPE: Self = Self("telescope");
    /// Settings menu interactor
    pub const SETTINGS: Self = Self("settings");
    /// Command line interactor (`:` commands)
    pub const COMMAND_LINE: Self = Self("command_line");
}

impl std::fmt::Display for InteractorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for InteractorId {
    fn default() -> Self {
        Self::EDITOR
    }
}

/// Result of handling input in an interactor
pub enum InputResult {
    /// Input was not handled
    NotHandled,
    /// Input was handled, request render
    Handled,
    /// Send this event to runtime (enlist pattern)
    SendEvent(InnerEvent),
}

/// Trait for components that handle user input
///
/// Implement this trait to create custom interactors that can be registered
/// with the [`InteractorRegistry`].
pub trait Interactor: std::fmt::Debug + Send + Sync {
    /// Unique identifier for this interactor
    fn id(&self) -> InteractorId;

    /// Display name for status line
    fn display_name(&self) -> &'static str;

    /// Handle character input
    fn handle_insert_char(&mut self, c: char, mode_state: &ModeState) -> InputResult;

    /// Handle backspace/delete backward
    fn handle_delete_backward(&mut self, mode_state: &ModeState) -> InputResult;

    /// Icon for status line (optional)
    fn icon(&self) -> Option<&'static str> {
        None
    }
}

/// Registry of interactors
///
/// Manages all registered interactors and tracks the currently active one.
#[derive(Debug)]
pub struct InteractorRegistry {
    targets: HashMap<InteractorId, Box<dyn Interactor>>,
    active: InteractorId,
}

impl Default for InteractorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractorRegistry {
    /// Create a new empty interactor registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            targets: HashMap::new(),
            active: InteractorId::EDITOR,
        }
    }

    /// Register an interactor
    ///
    /// If an interactor with the same ID already exists, it will be replaced.
    pub fn register(&mut self, target: Box<dyn Interactor>) {
        let id = target.id();
        self.targets.insert(id, target);
    }

    /// Set the active interactor
    ///
    /// Returns `true` if the focus was changed, `false` if the interactor ID is not registered.
    pub fn set_active(&mut self, id: InteractorId) -> bool {
        if self.targets.contains_key(&id) {
            self.active = id;
            true
        } else {
            false
        }
    }

    /// Get the active interactor
    ///
    /// # Panics
    ///
    /// Panics if the active interactor is not registered (should never happen in normal use).
    #[must_use]
    pub fn active(&self) -> &dyn Interactor {
        self.targets
            .get(&self.active)
            .map(AsRef::as_ref)
            .expect("active interactor should always be registered")
    }

    /// Get the active interactor mutably
    ///
    /// # Panics
    ///
    /// Panics if the active interactor is not registered.
    pub fn active_mut(&mut self) -> &mut Box<dyn Interactor> {
        self.targets
            .get_mut(&self.active)
            .expect("active interactor should always be registered")
    }

    /// Get the active interactor ID
    #[must_use]
    pub const fn active_id(&self) -> InteractorId {
        self.active
    }

    /// Get an interactor by ID
    #[must_use]
    pub fn get(&self, id: InteractorId) -> Option<&dyn Interactor> {
        self.targets.get(&id).map(AsRef::as_ref)
    }

    /// Get an interactor by ID mutably
    pub fn get_mut(&mut self, id: InteractorId) -> Option<&mut Box<dyn Interactor>> {
        self.targets.get_mut(&id)
    }

    /// Check if an interactor is registered
    #[must_use]
    pub fn contains(&self, id: InteractorId) -> bool {
        self.targets.contains_key(&id)
    }

    /// Get the number of registered interactors
    #[must_use]
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Create a registry with default interactors registered
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(Editor::default()));
        registry.register(Box::new(Explorer::default()));
        registry.register(Box::new(Telescope));
        registry.register(Box::new(Settings));
        registry.register(Box::new(CommandLineInt));
        registry
    }
}

// Built-in interactors

/// Editor interactor (main text editing area)
#[derive(Debug, Default)]
pub struct Editor {
    /// Clear landing page on next input
    pub clear_landing_on_input: bool,
}

impl Interactor for Editor {
    fn id(&self) -> InteractorId {
        InteractorId::EDITOR
    }

    fn display_name(&self) -> &'static str {
        "EDITOR"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn handle_insert_char(&mut self, c: char, mode_state: &ModeState) -> InputResult {
        if mode_state.is_command() || mode_state.is_insert() {
            let clear = std::mem::take(&mut self.clear_landing_on_input);
            InputResult::SendEvent(InnerEvent::FocusInput {
                char: Some(c),
                delete: false,
                clear_landing: clear,
            })
        } else {
            InputResult::NotHandled
        }
    }

    fn handle_delete_backward(&mut self, mode_state: &ModeState) -> InputResult {
        if mode_state.is_command() || mode_state.is_insert() {
            InputResult::SendEvent(InnerEvent::FocusInput {
                char: None,
                delete: true,
                clear_landing: false,
            })
        } else {
            InputResult::NotHandled
        }
    }
}

/// Explorer interactor (file browser sidebar)
///
/// Handles filename input for create/rename operations.
#[derive(Debug, Default)]
pub struct Explorer {
    /// Current input buffer for filename operations
    pub input_buffer: String,
    /// Whether input mode is active
    pub input_active: bool,
}

impl Interactor for Explorer {
    fn id(&self) -> InteractorId {
        InteractorId::EXPLORER
    }

    fn display_name(&self) -> &'static str {
        "EXPLORER"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn handle_insert_char(&mut self, c: char, _mode_state: &ModeState) -> InputResult {
        if self.input_active {
            self.input_buffer.push(c);
            InputResult::Handled
        } else {
            InputResult::NotHandled
        }
    }

    fn handle_delete_backward(&mut self, _mode_state: &ModeState) -> InputResult {
        if self.input_active {
            self.input_buffer.pop();
            InputResult::Handled
        } else {
            InputResult::NotHandled
        }
    }
}

/// Telescope interactor (fuzzy finder)
///
/// Marker struct - actual state is in `TelescopeState` on Runtime.
#[derive(Debug, Default)]
pub struct Telescope;

impl Interactor for Telescope {
    fn id(&self) -> InteractorId {
        InteractorId::TELESCOPE
    }

    fn display_name(&self) -> &'static str {
        "TELESCOPE"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn handle_insert_char(&mut self, c: char, _mode_state: &ModeState) -> InputResult {
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: Some(c),
            delete: false,
            clear_landing: false,
        })
    }

    fn handle_delete_backward(&mut self, _mode_state: &ModeState) -> InputResult {
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: None,
            delete: true,
            clear_landing: false,
        })
    }
}

/// Settings menu interactor
#[derive(Debug, Default)]
pub struct Settings;

impl Interactor for Settings {
    fn id(&self) -> InteractorId {
        InteractorId::SETTINGS
    }

    fn display_name(&self) -> &'static str {
        "SETTINGS"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn handle_insert_char(&mut self, _c: char, _mode_state: &ModeState) -> InputResult {
        InputResult::NotHandled
    }

    fn handle_delete_backward(&mut self, _mode_state: &ModeState) -> InputResult {
        InputResult::NotHandled
    }
}

/// Command line interactor (`:` command input)
///
/// Handles input for command-line mode. State is managed via the enlist pattern,
/// with the actual `CommandLine` struct on Runtime.
#[derive(Debug, Default)]
pub struct CommandLineInt;

impl Interactor for CommandLineInt {
    fn id(&self) -> InteractorId {
        InteractorId::COMMAND_LINE
    }

    fn display_name(&self) -> &'static str {
        "COMMAND"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("󰘳 ")
    }

    fn handle_insert_char(&mut self, c: char, _mode_state: &ModeState) -> InputResult {
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: Some(c),
            delete: false,
            clear_landing: false,
        })
    }

    fn handle_delete_backward(&mut self, _mode_state: &ModeState) -> InputResult {
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: None,
            delete: true,
            clear_landing: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interactor_id_equality() {
        assert_eq!(InteractorId::EDITOR, InteractorId("editor"));
        assert_ne!(InteractorId::EDITOR, InteractorId::EXPLORER);
    }

    #[test]
    fn test_interactor_registry_register() {
        let mut registry = InteractorRegistry::new();
        assert!(registry.is_empty());

        registry.register(Box::new(Editor::default()));
        assert_eq!(registry.len(), 1);
        assert!(registry.contains(InteractorId::EDITOR));
    }

    #[test]
    fn test_interactor_registry_active() {
        let mut registry = InteractorRegistry::new();
        registry.register(Box::new(Editor::default()));
        registry.register(Box::new(Telescope));

        assert_eq!(registry.active_id(), InteractorId::EDITOR);

        assert!(registry.set_active(InteractorId::TELESCOPE));
        assert_eq!(registry.active_id(), InteractorId::TELESCOPE);

        // Can't set to unregistered interactor
        assert!(!registry.set_active(InteractorId::EXPLORER));
        assert_eq!(registry.active_id(), InteractorId::TELESCOPE);
    }
}
