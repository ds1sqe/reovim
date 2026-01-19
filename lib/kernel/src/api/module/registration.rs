//! Registration types for commands, keybindings, and event handlers.

use {super::RegistrationFlags, crate::core::CommandId};

/// Command registration descriptor.
///
/// Linux equivalent: Like `struct file_operations` - declares command capabilities.
///
/// Fields align with `CommandTrait` in lib/core/src/command/traits.rs.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // Command capabilities use multiple bool flags
pub struct CommandRegistration {
    /// Unique command identifier (e.g., "delete", "yank", "motion:word").
    pub id: &'static str,
    /// Human-readable name for help display.
    pub name: &'static str,
    /// Description for help system.
    pub description: &'static str,
    /// Category for help grouping (e.g., "motion", "operator", "edit").
    pub category: Option<&'static str>,
    /// Whether command accepts a count prefix (e.g., 5j).
    pub accepts_count: bool,
    /// Whether command accepts a motion (e.g., dw, c$).
    pub accepts_motion: bool,
    /// Whether command is a "jump" (recorded in jump list).
    pub is_jump: bool,
    /// Whether command modifies buffer text (for undo grouping).
    pub is_text_modifying: bool,
    /// Dependencies on other commands (Linux: module dependencies).
    pub depends_on: &'static [&'static str],
    /// Registration flags.
    pub flags: RegistrationFlags,
}

impl CommandRegistration {
    /// Create a new command registration with required id.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self {
            id,
            name: "",
            description: "",
            category: None,
            accepts_count: false,
            accepts_motion: false,
            is_jump: false,
            is_text_modifying: false,
            depends_on: &[],
            flags: RegistrationFlags::new(),
        }
    }

    /// Set the display name.
    #[must_use]
    pub const fn with_name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    /// Set the description.
    #[must_use]
    pub const fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Set the category.
    #[must_use]
    pub const fn with_category(mut self, cat: &'static str) -> Self {
        self.category = Some(cat);
        self
    }

    /// Mark command as accepting a count.
    #[must_use]
    pub const fn with_count(mut self) -> Self {
        self.accepts_count = true;
        self
    }

    /// Mark command as accepting a motion.
    #[must_use]
    pub const fn with_motion(mut self) -> Self {
        self.accepts_motion = true;
        self
    }

    /// Mark command as a jump.
    #[must_use]
    pub const fn with_jump(mut self) -> Self {
        self.is_jump = true;
        self
    }

    /// Mark command as text-modifying.
    #[must_use]
    pub const fn with_text_modifying(mut self) -> Self {
        self.is_text_modifying = true;
        self
    }

    /// Set dependencies.
    #[must_use]
    pub const fn with_depends_on(mut self, deps: &'static [&'static str]) -> Self {
        self.depends_on = deps;
        self
    }

    /// Set registration flags.
    #[must_use]
    pub const fn with_flags(mut self, flags: RegistrationFlags) -> Self {
        self.flags = flags;
        self
    }
}

/// Keybinding registration descriptor.
///
/// Linux equivalent: Like `struct input_device_id` - declares key matching.
#[derive(Debug, Clone)]
pub struct KeybindingRegistration {
    /// Key sequence in vim notation (e.g., `"dd"`, `"<C-w>h"`, `"<Space>ff"`).
    pub keys: &'static str,
    /// Command ID to invoke (compile-time verified).
    pub command_id: CommandId,
    /// Modes where binding is active (e.g., `&["normal"]`, `&["normal", "visual"]`).
    /// Empty slice means all modes (like Linux's match-all).
    pub modes: &'static [&'static str],
    /// Description for which-key / help.
    pub description: &'static str,
    /// Category for which-key grouping (e.g., "window", "file", "search").
    pub category: Option<&'static str>,
    /// Whether binding is enabled (for conditional keybindings).
    pub enabled: bool,
    /// Priority for conflict resolution (lower = higher priority).
    /// Like Linux driver priority for matching.
    pub priority: u32,
    /// Dependencies on other keybindings or commands.
    pub depends_on: &'static [&'static str],
    /// Registration flags.
    pub flags: RegistrationFlags,
}

impl KeybindingRegistration {
    /// Create a new keybinding registration.
    ///
    /// The `command_id` parameter is a typed `CommandId`, enabling compile-time
    /// verification that the referenced command exists. Import command ID constants
    /// from the appropriate module (e.g., `reovim_module_editor::ids::CURSOR_DOWN`).
    #[must_use]
    pub const fn new(keys: &'static str, command_id: CommandId) -> Self {
        Self {
            keys,
            command_id,
            modes: &[],
            description: "",
            category: None,
            enabled: true,
            priority: 100, // Default plugin priority
            depends_on: &[],
            flags: RegistrationFlags::new(),
        }
    }

    /// Set active modes.
    #[must_use]
    pub const fn with_modes(mut self, modes: &'static [&'static str]) -> Self {
        self.modes = modes;
        self
    }

    /// Set description.
    #[must_use]
    pub const fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Set category.
    #[must_use]
    pub const fn with_category(mut self, cat: &'static str) -> Self {
        self.category = Some(cat);
        self
    }

    /// Disable the keybinding.
    #[must_use]
    pub const fn with_disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set dependencies.
    #[must_use]
    pub const fn with_depends_on(mut self, deps: &'static [&'static str]) -> Self {
        self.depends_on = deps;
        self
    }

    /// Set registration flags.
    #[must_use]
    pub const fn with_flags(mut self, flags: RegistrationFlags) -> Self {
        self.flags = flags;
        self
    }
}

/// Event handler registration descriptor.
///
/// Linux equivalent: Like `struct notifier_block` - declares event subscription.
///
/// Priority convention (from EventBus):
/// - 0-50: Core handlers (kernel-level)
/// - 100: Default plugin priority
/// - 200+: Cleanup/late handlers
#[derive(Debug, Clone)]
pub struct EventHandlerRegistration {
    /// Event type name (e.g., `BufferChanged`, `CursorMoved`).
    pub event_type: &'static str,
    /// Handler priority (lower = called earlier).
    pub priority: u32,
    /// Description for debugging/introspection.
    pub description: &'static str,
    /// Whether handler auto-unsubscribes after one event (one-shot).
    pub once: bool,
    /// Optional target component ID for scoped events.
    pub target_component: Option<&'static str>,
    /// Dependencies on other handlers or modules.
    pub depends_on: &'static [&'static str],
    /// Registration flags.
    pub flags: RegistrationFlags,
}

impl EventHandlerRegistration {
    /// Create a new event handler registration.
    #[must_use]
    pub const fn new(event_type: &'static str) -> Self {
        Self {
            event_type,
            priority: 100, // Default plugin priority
            description: "",
            once: false,
            target_component: None,
            depends_on: &[],
            flags: RegistrationFlags::new(),
        }
    }

    /// Set priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set description.
    #[must_use]
    pub const fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Mark as one-shot handler.
    #[must_use]
    pub const fn with_once(mut self) -> Self {
        self.once = true;
        self
    }

    /// Set target component.
    #[must_use]
    pub const fn with_target(mut self, component: &'static str) -> Self {
        self.target_component = Some(component);
        self
    }

    /// Set dependencies.
    #[must_use]
    pub const fn with_depends_on(mut self, deps: &'static [&'static str]) -> Self {
        self.depends_on = deps;
        self
    }

    /// Set registration flags.
    #[must_use]
    pub const fn with_flags(mut self, flags: RegistrationFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set core priority (clamped to 0-50 range).
    #[must_use]
    pub const fn core_priority(mut self, priority: u32) -> Self {
        self.priority = if priority > 50 { 50 } else { priority };
        self
    }
}

/// Empty session handler registration descriptor.
///
/// Modules return these from `empty_session_handlers()` to register
/// handlers for the empty session state.
///
/// Linux equivalent: Like a device driver's `probe()` registration -
/// declares what to do when a particular condition (empty session) is met.
///
/// # Priority Convention
///
/// - 0-50: Core handlers (system-level)
/// - 100: Default module priority
/// - 200+: Late/fallback handlers
#[derive(Debug, Clone)]
pub struct EmptySessionHandlerRegistration {
    /// Unique handler identifier (e.g., `"defaults:scratch-buffer"`).
    pub id: &'static str,

    /// Human-readable description.
    pub description: &'static str,

    /// Handler priority (lower = called first).
    pub priority: u32,

    /// Registration flags.
    pub flags: RegistrationFlags,
}

impl EmptySessionHandlerRegistration {
    /// Create a new registration with required id.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self {
            id,
            description: "",
            priority: 100,
            flags: RegistrationFlags::new(),
        }
    }

    /// Set description.
    #[must_use]
    pub const fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Set priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set core priority (clamped to 0-50).
    #[must_use]
    pub const fn core_priority(mut self, priority: u32) -> Self {
        self.priority = if priority > 50 { 50 } else { priority };
        self
    }

    /// Set registration flags.
    #[must_use]
    pub const fn with_flags(mut self, flags: RegistrationFlags) -> Self {
        self.flags = flags;
        self
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::api::module::ModuleId};

    // Test constants for keybinding tests
    const TEST_MODULE: ModuleId = ModuleId::new("test");
    const DELETE_LINE: CommandId = CommandId::new(TEST_MODULE, "delete-line");
    const WINDOW_LEFT: CommandId = CommandId::new(TEST_MODULE, "window-left");
    const GOTO_DEFINITION: CommandId = CommandId::new(TEST_MODULE, "goto-definition");

    #[test]
    fn test_command_registration_new() {
        let reg = CommandRegistration::new("delete");
        assert_eq!(reg.id, "delete");
        assert_eq!(reg.name, "");
        assert_eq!(reg.description, "");
        assert!(reg.category.is_none());
        assert!(!reg.accepts_count);
        assert!(!reg.accepts_motion);
        assert!(!reg.is_jump);
        assert!(!reg.is_text_modifying);
        assert!(reg.depends_on.is_empty());
    }

    #[test]
    fn test_command_registration_builder() {
        let reg = CommandRegistration::new("delete")
            .with_name("Delete")
            .with_description("Delete text")
            .with_category("operator")
            .with_count()
            .with_motion()
            .with_text_modifying()
            .with_depends_on(&["yank"])
            .with_flags(RegistrationFlags::required());

        assert_eq!(reg.id, "delete");
        assert_eq!(reg.name, "Delete");
        assert_eq!(reg.description, "Delete text");
        assert_eq!(reg.category, Some("operator"));
        assert!(reg.accepts_count);
        assert!(reg.accepts_motion);
        assert!(!reg.is_jump);
        assert!(reg.is_text_modifying);
        assert_eq!(reg.depends_on, &["yank"]);
        assert!(reg.flags.required);
    }

    #[test]
    fn test_command_registration_jump() {
        let reg = CommandRegistration::new("goto-definition").with_jump();
        assert!(reg.is_jump);
    }

    #[test]
    fn test_keybinding_registration_new() {
        let reg = KeybindingRegistration::new("dd", DELETE_LINE);
        assert_eq!(reg.keys, "dd");
        assert_eq!(reg.command_id, DELETE_LINE);
        assert!(reg.modes.is_empty()); // All modes
        assert_eq!(reg.description, "");
        assert!(reg.category.is_none());
        assert!(reg.enabled);
        assert_eq!(reg.priority, 100); // Default plugin priority
        assert!(reg.depends_on.is_empty());
    }

    #[test]
    fn test_keybinding_registration_builder() {
        let reg = KeybindingRegistration::new("<C-w>h", WINDOW_LEFT)
            .with_modes(&["normal"])
            .with_description("Move to left window")
            .with_category("window")
            .with_priority(50)
            .with_depends_on(&["window-split"])
            .with_flags(RegistrationFlags::deferrable());

        assert_eq!(reg.keys, "<C-w>h");
        assert_eq!(reg.command_id, WINDOW_LEFT);
        assert_eq!(reg.modes, &["normal"]);
        assert_eq!(reg.description, "Move to left window");
        assert_eq!(reg.category, Some("window"));
        assert!(reg.enabled);
        assert_eq!(reg.priority, 50);
        assert_eq!(reg.depends_on, &["window-split"]);
        assert!(reg.flags.deferrable);
    }

    #[test]
    fn test_keybinding_registration_disabled() {
        let reg = KeybindingRegistration::new("gd", GOTO_DEFINITION).with_disabled();
        assert!(!reg.enabled);
    }

    #[test]
    fn test_event_handler_registration_new() {
        let reg = EventHandlerRegistration::new("BufferChanged");
        assert_eq!(reg.event_type, "BufferChanged");
        assert_eq!(reg.priority, 100); // Default plugin priority
        assert_eq!(reg.description, "");
        assert!(!reg.once);
        assert!(reg.target_component.is_none());
        assert!(reg.depends_on.is_empty());
    }

    #[test]
    fn test_event_handler_registration_builder() {
        let reg = EventHandlerRegistration::new("CursorMoved")
            .with_priority(50)
            .with_description("Update cursor highlight")
            .with_target("treesitter")
            .with_depends_on(&["syntax-highlight"])
            .with_flags(RegistrationFlags::deferrable());

        assert_eq!(reg.event_type, "CursorMoved");
        assert_eq!(reg.priority, 50);
        assert_eq!(reg.description, "Update cursor highlight");
        assert!(!reg.once);
        assert_eq!(reg.target_component, Some("treesitter"));
        assert_eq!(reg.depends_on, &["syntax-highlight"]);
        assert!(reg.flags.deferrable);
    }

    #[test]
    fn test_event_handler_registration_once() {
        let reg = EventHandlerRegistration::new("ModuleLoaded").with_once();
        assert!(reg.once);
    }

    #[test]
    fn test_event_handler_core_priority() {
        // Core priority clamped to 0-50
        let reg = EventHandlerRegistration::new("BufferChanged").core_priority(25);
        assert_eq!(reg.priority, 25);

        let reg = EventHandlerRegistration::new("BufferChanged").core_priority(100);
        assert_eq!(reg.priority, 50); // Clamped

        let reg = EventHandlerRegistration::new("BufferChanged").core_priority(0);
        assert_eq!(reg.priority, 0);
    }

    #[test]
    fn test_empty_session_handler_registration_new() {
        let reg = EmptySessionHandlerRegistration::new("scratch-buffer:handler");
        assert_eq!(reg.id, "scratch-buffer:handler");
        assert_eq!(reg.description, "");
        assert_eq!(reg.priority, 100); // Default module priority
    }

    #[test]
    fn test_empty_session_handler_registration_builder() {
        let reg = EmptySessionHandlerRegistration::new("defaults:scratch-buffer")
            .with_description("Create empty scratch buffer on startup")
            .with_priority(100)
            .with_flags(RegistrationFlags::deferrable());

        assert_eq!(reg.id, "defaults:scratch-buffer");
        assert_eq!(reg.description, "Create empty scratch buffer on startup");
        assert_eq!(reg.priority, 100);
        assert!(reg.flags.deferrable);
    }

    #[test]
    fn test_empty_session_handler_core_priority() {
        // Core priority clamped to 0-50
        let reg = EmptySessionHandlerRegistration::new("core:init").core_priority(25);
        assert_eq!(reg.priority, 25);

        let reg = EmptySessionHandlerRegistration::new("core:init").core_priority(100);
        assert_eq!(reg.priority, 50); // Clamped

        let reg = EmptySessionHandlerRegistration::new("core:init").core_priority(0);
        assert_eq!(reg.priority, 0);
    }
}
