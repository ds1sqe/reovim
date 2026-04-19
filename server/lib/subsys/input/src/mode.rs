//! Mode keybinding types.
//!
//! This module defines keybinding and key sequence types.
//!
//! # Architecture
//!
//! | Layer | Responsibility |
//! |-------|---------------|
//! | Kernel | Identity: `Mode` trait, `ModeId`, `CommandId` |
//! | Input Driver (this) | Input: `KeySequence`, `Keybinding` |
//! | Modules | Policy: actual mode implementations and keybindings |
//!
//! # Example
//!
//! ```ignore
//! use reovim_subsys_input::{KeySequence, Keybinding, KeyEvent, KeyCode};
//! use reovim_kernel::api::v1::{ModeId, CommandId, ModuleId};
//!
//! // Create a keybinding for "j" in normal mode
//! let module = ModuleId::new("editor");
//! let normal = ModeId::new(module.clone(), "normal");
//! let cursor_down = CommandId::new(module, "cursor-down");
//!
//! let binding = Keybinding::new(
//!     KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('j'))]),
//!     normal,
//!     cursor_down,
//!     "Move cursor down",
//! );
//! ```

use reovim_kernel::api::v1::{CommandId, ModeId};

pub use reovim_subsys_input_contracts::KeySequence;

// ============================================================================
// KeybindingTarget
// ============================================================================

/// What a keybinding triggers.
///
/// Keybindings can either execute a command or enter a mode.
/// This enables user-configurable mode bindings like `<C-t>` to enter Tetromino mode.
///
/// # TOML Syntax
///
/// ```toml
/// [bindings.normal]
/// "dd" = "editor:delete-line"      # Command (default)
/// "i" = { enter = "vim:insert" }   # Mode (explicit)
/// "<C-t>" = { enter = "tetromino:play" }
/// ```
///
/// # Example
///
/// ```ignore
/// use reovim_subsys_input::KeybindingTarget;
/// use reovim_kernel::api::v1::{CommandId, ModeId, ModuleId};
///
/// let module = ModuleId::new("editor");
///
/// // Command target
/// let cmd = CommandId::new(module.clone(), "delete-line");
/// let target = KeybindingTarget::Command(cmd);
///
/// // Mode target
/// let mode = ModeId::new(module, "insert");
/// let target = KeybindingTarget::EnterMode(mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindingTarget {
    /// Execute a command.
    Command(CommandId),
    /// Enter a mode (push onto mode stack).
    EnterMode(ModeId),
}

impl KeybindingTarget {
    /// Check if this is a command target.
    #[must_use]
    pub const fn is_command(&self) -> bool {
        matches!(self, Self::Command(_))
    }

    /// Check if this is a mode target.
    #[must_use]
    pub const fn is_mode(&self) -> bool {
        matches!(self, Self::EnterMode(_))
    }

    /// Get the command if this is a command target.
    #[must_use]
    pub const fn as_command(&self) -> Option<&CommandId> {
        match self {
            Self::Command(cmd) => Some(cmd),
            Self::EnterMode(_) => None,
        }
    }

    /// Get the mode if this is a mode target.
    #[must_use]
    pub const fn as_mode(&self) -> Option<&ModeId> {
        match self {
            Self::Command(_) => None,
            Self::EnterMode(mode) => Some(mode),
        }
    }
}

impl From<CommandId> for KeybindingTarget {
    fn from(cmd: CommandId) -> Self {
        Self::Command(cmd)
    }
}

impl From<ModeId> for KeybindingTarget {
    fn from(mode: ModeId) -> Self {
        Self::EnterMode(mode)
    }
}

impl std::fmt::Display for KeybindingTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(cmd) => write!(f, "{cmd}"),
            Self::EnterMode(mode) => write!(f, "enter:{mode}"),
        }
    }
}

// ============================================================================
// Keybinding
// ============================================================================

/// A keybinding maps a key sequence in a mode to a command.
///
/// Keybindings are the fundamental unit of the keymap system. They specify:
/// - What keys trigger the binding
/// - Which mode the binding is active in
/// - What command to execute
/// - A human-readable description
///
/// # Example
///
/// ```ignore
/// use reovim_subsys_input::{Keybinding, KeySequence, KeyEvent, KeyCode};
/// use reovim_kernel::api::v1::{ModeId, CommandId, ModuleId};
///
/// let module = ModuleId::new("editor");
/// let normal = ModeId::new(module.clone(), "normal");
/// let cmd = CommandId::new(module, "cursor-down");
///
/// let binding = Keybinding::new(
///     KeySequence::parse("j").unwrap(),
///     normal,
///     cmd,
///     "Move cursor down",
/// );
/// ```
#[derive(Debug, Clone)]
pub struct Keybinding {
    /// The key sequence that triggers this binding.
    pub keys: KeySequence,
    /// The mode in which this binding is active.
    pub mode: ModeId,
    /// The command to execute when triggered.
    pub command: CommandId,
    /// Human-readable description of the binding.
    pub description: &'static str,
}

impl Keybinding {
    /// Create a new keybinding.
    #[must_use]
    pub const fn new(
        keys: KeySequence,
        mode: ModeId,
        command: CommandId,
        description: &'static str,
    ) -> Self {
        Self {
            keys,
            mode,
            command,
            description,
        }
    }

    /// Create a keybinding from a string notation.
    ///
    /// # Errors
    ///
    /// Returns `None` if the key string is invalid.
    #[must_use]
    pub fn from_str(
        keys: &str,
        mode: ModeId,
        command: CommandId,
        description: &'static str,
    ) -> Option<Self> {
        let keys = KeySequence::parse(keys)?;
        Some(Self {
            keys,
            mode,
            command,
            description,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================
