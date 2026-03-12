//! Registration types for commands, keybindings, and event handlers.

use {super::RegistrationFlags, crate::core::CommandId};

/// Bit position for "accepts count" capability.
const CAP_ACCEPTS_COUNT: u8 = 1 << 0;
/// Bit position for "accepts motion" capability.
const CAP_ACCEPTS_MOTION: u8 = 1 << 1;
/// Bit position for "is jump" capability.
const CAP_IS_JUMP: u8 = 1 << 2;
/// Bit position for "is text-modifying" capability.
const CAP_IS_TEXT_MODIFYING: u8 = 1 << 3;

/// Command registration descriptor.
///
/// Linux equivalent: Like `struct file_operations` - declares command capabilities.
///
/// Fields align with `CommandTrait` from the original design (see `archive/pre_kernel/lib/core/src/command/traits.rs`).
///
/// Command capabilities (accepts count, accepts motion, is jump, is text-modifying)
/// are stored as a `u8` bitfield to preserve kernel purity (zero external deps).
#[derive(Debug, Clone)]
pub struct CommandRegistration {
    /// Unique command identifier (e.g., "delete", "yank", "motion:word").
    pub id: &'static str,
    /// Human-readable name for help display.
    pub name: &'static str,
    /// Description for help system.
    pub description: &'static str,
    /// Category for help grouping (e.g., "motion", "operator", "edit").
    pub category: Option<&'static str>,
    /// Command capability flags (`accepts_count`, `accepts_motion`, `is_jump`, `is_text_modifying`).
    capabilities: u8,
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
            capabilities: 0,
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
        self.capabilities |= CAP_ACCEPTS_COUNT;
        self
    }

    /// Mark command as accepting a motion.
    #[must_use]
    pub const fn with_motion(mut self) -> Self {
        self.capabilities |= CAP_ACCEPTS_MOTION;
        self
    }

    /// Mark command as a jump.
    #[must_use]
    pub const fn with_jump(mut self) -> Self {
        self.capabilities |= CAP_IS_JUMP;
        self
    }

    /// Mark command as text-modifying.
    #[must_use]
    pub const fn with_text_modifying(mut self) -> Self {
        self.capabilities |= CAP_IS_TEXT_MODIFYING;
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

    // ========================================================================
    // Query Methods
    // ========================================================================

    /// Check if this command accepts a count prefix (e.g., 5j).
    #[must_use]
    pub const fn accepts_count(&self) -> bool {
        self.capabilities & CAP_ACCEPTS_COUNT != 0
    }

    /// Check if this command accepts a motion (e.g., dw, c$).
    #[must_use]
    pub const fn accepts_motion(&self) -> bool {
        self.capabilities & CAP_ACCEPTS_MOTION != 0
    }

    /// Check if this command is a "jump" (recorded in jump list).
    #[must_use]
    pub const fn is_jump(&self) -> bool {
        self.capabilities & CAP_IS_JUMP != 0
    }

    /// Check if this command modifies buffer text (for undo grouping).
    #[must_use]
    pub const fn is_text_modifying(&self) -> bool {
        self.capabilities & CAP_IS_TEXT_MODIFYING != 0
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
