//! Policy interface traits.
//!
//! These traits define contracts that **MODULES** implement (policy).
//! The kernel provides mechanisms; modules provide behavior.
//!
//! # Design Principle
//!
//! > *"Provide mechanism, not policy."* — Unix Philosophy
//!
//! | Layer | Responsibility | Example |
//! |-------|---------------|---------|
//! | **Kernel (Mechanism)** | WHAT can be done | `MotionEngine::calculate(...)` |
//! | **Module (Policy)** | HOW it should be done | `'j' → cursor down` |
//!
//! # Traits
//!
//! - [`Operator`] - Executes actions on text ranges (delete, yank, change)
//! - [`KeymapProvider`] - Resolves key sequences to actions
//! - [`CommandHandler`] - Handles ex-commands (:w, :q, :set)
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::api::v1::*;
//!
//! // Module implements Operator trait (policy)
//! struct DeleteOperator;
//!
//! impl Operator for DeleteOperator {
//!     fn id(&self) -> &'static str { "delete" }
//!
//!     fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range)
//!         -> Result<(), OperatorError>
//!     {
//!         // Use kernel mechanisms to delete text
//!         let buffer = ctx.kernel.buffers.get(ctx.buffer_id)?;
//!         buffer.write().delete_range(range)?;
//!         Ok(())
//!     }
//! }
//! ```

use crate::mm::{BufferId, Position};

use super::{context::KernelContext, window_manager::WindowId};

// ============================================================================
// Range Type
// ============================================================================

/// Text range for operator execution.
///
/// Represents a contiguous range of text in a buffer.
/// Used by operators to know which text to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive).
    pub end: Position,
}

impl Range {
    /// Create a new range.
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Create a range from a single position (zero-width).
    #[must_use]
    pub const fn from_position(pos: Position) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    /// Check if the range is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start.line == self.end.line && self.start.column == self.end.column
    }

    /// Check if this is a single-line range.
    #[must_use]
    pub const fn is_single_line(&self) -> bool {
        self.start.line == self.end.line
    }

    /// Get the number of lines spanned.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        if self.end.line >= self.start.line {
            self.end.line - self.start.line + 1
        } else {
            0
        }
    }

    /// Normalize the range so start <= end.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Comparison logic
    pub fn normalized(self) -> Self {
        if self.start.line > self.end.line
            || (self.start.line == self.end.line && self.start.column > self.end.column)
        {
            Self {
                start: self.end,
                end: self.start,
            }
        } else {
            self
        }
    }
}

// ============================================================================
// Operator Trait
// ============================================================================

/// Operators execute actions on text ranges.
///
/// - **Mechanism (Kernel)**: Range calculation, text manipulation APIs
/// - **Policy (Module)**: What each operator does with the range
///
/// # Examples
///
/// - `delete` - Remove text in range, save to register
/// - `yank` - Copy text in range to register
/// - `change` - Delete text and enter insert mode
/// - `indent` - Increase indentation of lines in range
///
/// # Implementation
///
/// Modules implement this trait to define operator behavior:
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// struct YankOperator;
///
/// impl Operator for YankOperator {
///     fn id(&self) -> &'static str { "yank" }
///
///     fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range)
///         -> Result<(), OperatorError>
///     {
///         let buffer = ctx.kernel.buffers.get(ctx.buffer_id)
///             .ok_or(OperatorError::BufferNotFound(ctx.buffer_id))?;
///
///         let text = buffer.read().text_in_range(&range);
///         let register = ctx.register.unwrap_or('"');
///         // Save to register...
///         Ok(())
///     }
///
///     fn is_text_modifying(&self) -> bool { false } // yank doesn't modify
/// }
/// ```
pub trait Operator: Send + Sync {
    /// Unique identifier for this operator.
    fn id(&self) -> &'static str;

    /// Execute the operator on a range.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Execution context with kernel access
    /// * `range` - Text range to operate on
    ///
    /// # Errors
    ///
    /// Returns `OperatorError` if the operation fails.
    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError>;

    /// Whether this operator works on whole lines (dd, yy).
    ///
    /// Linewise operators extend the range to include full lines.
    fn is_linewise(&self) -> bool {
        false
    }

    /// Whether this operator modifies text.
    ///
    /// Used for undo grouping - modifying operators create checkpoints.
    fn is_text_modifying(&self) -> bool {
        true
    }
}

/// Context passed to operator execution.
pub struct OperatorContext<'a> {
    /// Kernel context for accessing services.
    pub kernel: &'a KernelContext,
    /// Buffer to operate on.
    pub buffer_id: BufferId,
    /// Target register (e.g., `"a` for register 'a').
    pub register: Option<char>,
    /// Count prefix (e.g., `3dd` has count 3).
    pub count: usize,
}

/// Operator execution errors.
#[derive(Debug, Clone)]
pub enum OperatorError {
    /// Buffer not found.
    BufferNotFound(BufferId),
    /// Invalid range.
    InvalidRange(Range),
    /// Operation failed.
    OperationFailed(String),
}

impl std::fmt::Display for OperatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferNotFound(id) => write!(f, "buffer not found: {id:?}"),
            Self::InvalidRange(r) => write!(f, "invalid range: {r:?}"),
            Self::OperationFailed(msg) => write!(f, "operation failed: {msg}"),
        }
    }
}

impl std::error::Error for OperatorError {}

// ============================================================================
// KeymapProvider Trait
// ============================================================================

/// Resolves key sequences to actions.
///
/// - **Mechanism (Kernel)**: Key event delivery
/// - **Policy (Module)**: Which keys trigger which commands
///
/// # Implementation
///
/// Modules implement this trait to define keybindings:
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// struct VimKeymap;
///
/// impl KeymapProvider for VimKeymap {
///     fn resolve(&self, mode: &str, keys: &[KeyEvent]) -> KeymapResult {
///         if mode == "normal" && keys.len() == 1 {
///             if let KeyCode::Char('j') = keys[0].code {
///                 return KeymapResult::Matched {
///                     command_id: "cursor-down".into(),
///                     count: None,
///                 };
///             }
///         }
///         KeymapResult::NotMatched
///     }
///
///     fn bindings_for_mode(&self, mode: &str) -> Vec<KeybindingInfo> {
///         // Return all bindings for which-key display
///         vec![]
///     }
/// }
/// ```
pub trait KeymapProvider: Send + Sync {
    /// Resolve a key sequence in a given mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Current mode (e.g., "normal", "insert", "visual")
    /// * `keys` - Key sequence to resolve
    ///
    /// # Returns
    ///
    /// - `Matched` - Key sequence matched a command
    /// - `Pending` - More keys needed to complete sequence
    /// - `NotMatched` - No binding found
    fn resolve(&self, mode: &str, keys: &[KeyEvent]) -> KeymapResult;

    /// Get all keybindings for a mode.
    ///
    /// Used by which-key to display available bindings.
    fn bindings_for_mode(&self, mode: &str) -> Vec<KeybindingInfo>;
}

/// Result of key sequence resolution.
#[derive(Debug, Clone)]
pub enum KeymapResult {
    /// Key sequence matched a command.
    Matched {
        /// Command identifier to execute.
        command_id: String,
        /// Count prefix (e.g., "3j" has count Some(3)).
        count: Option<usize>,
    },
    /// More keys needed to complete sequence.
    ///
    /// For example, after 'd' we're waiting for a motion.
    Pending,
    /// Key sequence doesn't match any binding.
    NotMatched,
}

/// Information about a keybinding.
#[derive(Debug, Clone)]
pub struct KeybindingInfo {
    /// Key sequence string (e.g., "dd", "<C-w>h").
    pub keys: String,
    /// Command identifier.
    pub command_id: String,
    /// Human-readable description.
    pub description: String,
    /// Optional category for grouping.
    pub category: Option<String>,
}

/// Key event for keymap resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// The key code.
    pub code: KeyCode,
    /// Key modifiers.
    pub modifiers: Modifiers,
}

impl KeyEvent {
    /// Create a new key event.
    #[must_use]
    pub const fn new(code: KeyCode, modifiers: Modifiers) -> Self {
        Self { code, modifiers }
    }

    /// Create a simple key event with no modifiers.
    #[must_use]
    pub const fn char(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: Modifiers::NONE,
        }
    }
}

/// Key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// Character key.
    Char(char),
    /// Enter/Return.
    Enter,
    /// Escape.
    Escape,
    /// Backspace.
    Backspace,
    /// Tab.
    Tab,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Delete.
    Delete,
    /// Insert.
    Insert,
    /// Function key (F1-F12).
    F(u8),
}

/// Key modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Control key.
    pub ctrl: bool,
    /// Alt/Option key.
    pub alt: bool,
    /// Shift key.
    pub shift: bool,
}

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
    };

    /// Control modifier.
    pub const CTRL: Self = Self {
        ctrl: true,
        alt: false,
        shift: false,
    };

    /// Alt modifier.
    pub const ALT: Self = Self {
        ctrl: false,
        alt: true,
        shift: false,
    };

    /// Shift modifier.
    pub const SHIFT: Self = Self {
        ctrl: false,
        alt: false,
        shift: true,
    };
}

// ============================================================================
// CommandHandler Trait
// ============================================================================

/// Handles ex-commands (commands entered via :).
///
/// - **Mechanism (Kernel)**: Command parsing, execution context
/// - **Policy (Module)**: What :w, :q, :set actually do
///
/// # Implementation
///
/// Modules implement this trait to define command behavior:
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// struct WriteCommand;
///
/// impl CommandHandler for WriteCommand {
///     fn id(&self) -> &'static str { "write" }
///     fn names(&self) -> &[&'static str] { &["w", "write"] }
///
///     fn execute(&self, ctx: &mut CommandContext<'_>, args: &[&str])
///         -> Result<(), CommandError>
///     {
///         let buffer_id = ctx.buffer_id
///             .ok_or(CommandError::NoBuffer)?;
///
///         let path = if args.is_empty() {
///             // Use buffer's existing path
///             None
///         } else {
///             Some(args[0])
///         };
///
///         // Write the buffer to disk...
///         Ok(())
///     }
///
///     fn help(&self) -> &'static str {
///         "Write the current buffer to disk"
///     }
/// }
/// ```
pub trait CommandHandler: Send + Sync {
    /// Command identifier.
    fn id(&self) -> &'static str;

    /// Command names (e.g., `["w", "write"]`).
    ///
    /// The first name is the canonical name.
    fn names(&self) -> &[&'static str];

    /// Execute the command.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Execution context
    /// * `args` - Command arguments (e.g., `:w foo.txt` has args `["foo.txt"]`)
    ///
    /// # Errors
    ///
    /// Returns `CommandError` if the command fails.
    fn execute(&self, ctx: &mut CommandContext<'_>, args: &[&str]) -> Result<(), CommandError>;

    /// Command completion suggestions.
    ///
    /// Returns possible completions for the given partial input.
    fn complete(&self, _partial: &str) -> Vec<String> {
        vec![]
    }

    /// Help text for the command.
    fn help(&self) -> &'static str {
        ""
    }
}

/// Context passed to command execution.
pub struct CommandContext<'a> {
    /// Kernel context for accessing services.
    pub kernel: &'a KernelContext,
    /// Current buffer (if any).
    pub buffer_id: Option<BufferId>,
    /// Current window (if any).
    pub window_id: Option<WindowId>,
    /// Whether command was invoked with ! (e.g., :q!).
    pub bang: bool,
    /// Command range (e.g., :1,5d has range Some((1,5))).
    pub range: Option<Range>,
}

/// Command execution errors.
#[derive(Debug, Clone)]
pub enum CommandError {
    /// No buffer available.
    NoBuffer,
    /// No window available.
    NoWindow,
    /// Invalid arguments.
    InvalidArguments(String),
    /// Execution failed.
    ExecutionFailed(String),
    /// Unknown command.
    UnknownCommand(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBuffer => write!(f, "no buffer"),
            Self::NoWindow => write!(f, "no window"),
            Self::InvalidArguments(msg) => write!(f, "invalid arguments: {msg}"),
            Self::ExecutionFailed(msg) => write!(f, "execution failed: {msg}"),
            Self::UnknownCommand(name) => write!(f, "unknown command: {name}"),
        }
    }
}

impl std::error::Error for CommandError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_new() {
        let range = Range::new(Position::new(0, 0), Position::new(1, 5));
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(1, 5));
    }

    #[test]
    fn test_range_is_empty() {
        let empty = Range::from_position(Position::new(5, 10));
        assert!(empty.is_empty());

        let not_empty = Range::new(Position::new(0, 0), Position::new(0, 1));
        assert!(!not_empty.is_empty());
    }

    #[test]
    fn test_range_line_count() {
        let single = Range::new(Position::new(5, 0), Position::new(5, 10));
        assert!(single.is_single_line());
        assert_eq!(single.line_count(), 1);

        let multi = Range::new(Position::new(0, 0), Position::new(3, 0));
        assert!(!multi.is_single_line());
        assert_eq!(multi.line_count(), 4);
    }

    #[test]
    fn test_range_normalized() {
        // Already normalized
        let range = Range::new(Position::new(0, 0), Position::new(1, 5));
        let norm = range.normalized();
        assert_eq!(norm.start, Position::new(0, 0));
        assert_eq!(norm.end, Position::new(1, 5));

        // Needs normalization
        let reversed = Range::new(Position::new(1, 5), Position::new(0, 0));
        let norm = reversed.normalized();
        assert_eq!(norm.start, Position::new(0, 0));
        assert_eq!(norm.end, Position::new(1, 5));
    }

    #[test]
    fn test_keymap_result() {
        let matched = KeymapResult::Matched {
            command_id: "cursor-down".into(),
            count: Some(3),
        };
        assert!(matches!(matched, KeymapResult::Matched { .. }));

        let pending = KeymapResult::Pending;
        assert!(matches!(pending, KeymapResult::Pending));
    }

    #[test]
    fn test_key_event() {
        let key = KeyEvent::char('j');
        assert_eq!(key.code, KeyCode::Char('j'));
        assert_eq!(key.modifiers, Modifiers::NONE);

        let ctrl_w = KeyEvent::new(KeyCode::Char('w'), Modifiers::CTRL);
        assert!(ctrl_w.modifiers.ctrl);
    }

    #[test]
    fn test_command_error_display() {
        let err = CommandError::NoBuffer;
        assert_eq!(err.to_string(), "no buffer");

        let err = CommandError::UnknownCommand("foo".into());
        assert!(err.to_string().contains("unknown command"));
    }
}
