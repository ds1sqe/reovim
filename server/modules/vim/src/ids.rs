//! Command and operator ID constants for the vim module.
//!
//! The vim module defines mode-switching commands, operators, and vim-specific
//! behavior. These constants enable compile-time verification of IDs
//! referenced in keybindings.
//!
//! # Operators
//!
//! Vim operators (d, y, c) are vim-specific policy, not kernel mechanism.
//! The kernel has ZERO vim knowledge - operators live here in the vim module.
//!
//! ```
//! use reovim_module_vim::ids::{OperatorId, DELETE, YANK, CHANGE};
//!
//! assert_eq!(DELETE.name(), "delete");
//! assert_eq!(YANK.name(), "yank");
//! assert_eq!(CHANGE.name(), "change");
//! ```

use std::fmt;

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Vim module ID.
pub const MODULE: ModuleId = ModuleId::new("vim");

// =============================================================================
// Operator ID Type
// =============================================================================

/// Namespaced operator identifier.
///
/// Operators are identified by their owning module and a local name.
/// This prevents naming conflicts between modules and provides type-safe
/// operator references instead of string-based identification.
///
/// # Note
///
/// Operators (d, y, c) are vim-specific policy. The kernel has ZERO vim
/// knowledge - this type lives in the vim module, not the kernel.
///
/// # Example
///
/// ```
/// use reovim_module_vim::ids::{OperatorId, MODULE, DELETE};
///
/// let delete = OperatorId::new(MODULE, "delete");
/// assert_eq!(delete.name(), "delete");
/// assert_eq!(delete.module().as_str(), "vim");
///
/// // Or use the pre-defined constant
/// assert_eq!(DELETE.name(), "delete");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OperatorId {
    /// The module that owns this operator.
    module: ModuleId,
    /// The local name within the module.
    name: &'static str,
}

impl OperatorId {
    /// Create a new operator identifier.
    #[must_use]
    pub const fn new(module: ModuleId, name: &'static str) -> Self {
        Self { module, name }
    }

    /// Create an operator identifier from a qualified string like "module:operator".
    ///
    /// This method is intended for dynamic use cases like FFI where operator IDs
    /// are specified as strings at runtime. The strings are leaked to get
    /// `'static` lifetime, so this should only be used for long-lived operators.
    ///
    /// If the string doesn't contain ':', the entire string is treated as the
    /// operator name with "unknown" as the module.
    #[must_use]
    pub fn from_qualified_leaked(qualified: String) -> Self {
        let (module_str, name_str) = if let Some(idx) = qualified.find(':') {
            (qualified[..idx].to_string(), qualified[idx + 1..].to_string())
        } else {
            ("unknown".to_string(), qualified)
        };

        let module_static: &'static str = Box::leak(module_str.into_boxed_str());
        let name_static: &'static str = Box::leak(name_str.into_boxed_str());

        Self {
            module: ModuleId::new(module_static),
            name: name_static,
        }
    }

    /// Get the owning module.
    #[must_use]
    pub const fn module(&self) -> &ModuleId {
        &self.module
    }

    /// Get the local name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl fmt::Display for OperatorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.module, self.name)
    }
}

// =============================================================================
// Operator Constants
// =============================================================================

/// Delete operator - removes text and saves to register.
///
/// Vim equivalent: `d`
pub const DELETE: OperatorId = OperatorId::new(MODULE, "delete");

/// Yank operator - copies text to register without modification.
///
/// Vim equivalent: `y`
pub const YANK: OperatorId = OperatorId::new(MODULE, "yank");

/// Change operator - deletes text and enters insert mode.
///
/// Vim equivalent: `c`
pub const CHANGE: OperatorId = OperatorId::new(MODULE, "change");

// =============================================================================
// Mode Switching - Insert
// =============================================================================

/// Enter insert mode (i).
pub const ENTER_INSERT: CommandId = CommandId::new(MODULE, "enter-insert");

/// Enter insert mode after cursor (a).
pub const ENTER_INSERT_AFTER: CommandId = CommandId::new(MODULE, "enter-insert-after");

/// Enter insert mode at end of line (A).
pub const ENTER_INSERT_EOL: CommandId = CommandId::new(MODULE, "enter-insert-eol");

/// Enter insert mode at first non-blank (I).
pub const ENTER_INSERT_BOL: CommandId = CommandId::new(MODULE, "enter-insert-bol");

/// Open line below and enter insert (o).
pub const OPEN_LINE_BELOW: CommandId = CommandId::new(MODULE, "open-line-below");

/// Open line above and enter insert (O).
pub const OPEN_LINE_ABOVE: CommandId = CommandId::new(MODULE, "open-line-above");

/// Exit insert mode to normal (Esc).
pub const EXIT_INSERT: CommandId = CommandId::new(MODULE, "exit-insert");

// =============================================================================
// Mode Switching - Visual
// =============================================================================

/// Enter visual mode (v).
pub const ENTER_VISUAL: CommandId = CommandId::new(MODULE, "enter-visual");

/// Enter visual line mode (V).
pub const ENTER_VISUAL_LINE: CommandId = CommandId::new(MODULE, "enter-visual-line");

/// Enter visual block mode (Ctrl-v).
pub const ENTER_VISUAL_BLOCK: CommandId = CommandId::new(MODULE, "enter-visual-block");

/// Exit visual mode (Esc).
pub const EXIT_VISUAL: CommandId = CommandId::new(MODULE, "exit-visual");

// =============================================================================
// Mode Switching - Other
// =============================================================================

/// Enter command-line mode (:).
pub const ENTER_COMMANDLINE: CommandId = CommandId::new(MODULE, "enter-commandline");

/// Cancel command-line mode without executing (Esc).
pub const CANCEL_COMMANDLINE: CommandId = CommandId::new(MODULE, "cancel-commandline");

/// Execute command-line and exit (Enter).
pub const EXIT_COMMANDLINE: CommandId = CommandId::new(MODULE, "exit-commandline");

/// Enter window mode (Ctrl-w).
pub const ENTER_WINDOW_MODE: CommandId = CommandId::new(MODULE, "enter-window-mode");

/// Cancel and return to normal mode (no cursor adjustment).
///
/// Used by operator modes (delete, yank, change) when escape is pressed.
/// Unlike `EXIT_INSERT`, this does not adjust cursor position.
pub const CANCEL_TO_NORMAL: CommandId = CommandId::new(MODULE, "cancel-to-normal");

// =============================================================================
// Search Mode Entry (#435)
// =============================================================================

/// Enter search forward mode (/).
///
/// Sets pending search direction to Forward and enters command-line mode.
pub const ENTER_SEARCH_FORWARD: CommandId = CommandId::new(MODULE, "enter-search-forward");

/// Enter search backward mode (?).
///
/// Sets pending search direction to Backward and enters command-line mode.
pub const ENTER_SEARCH_BACKWARD: CommandId = CommandId::new(MODULE, "enter-search-backward");

// =============================================================================
// Find-Char Motion Execution (Epic #385 - Resolver-based)
// =============================================================================

/// Execute find-char motion with character from context.
///
/// This command is called by the resolver when `pending_char` is set and a
/// character key is pressed. The character and direction are passed via
/// command context metadata:
/// - `find_char`: The target character (`ArgValue::Char`)
/// - `find_direction`: "forward" or "backward" (`ArgValue::String`)
/// - `find_inclusive`: true for f/F, false for t/T (`ArgValue::Bool`)
pub const EXECUTE_FIND_CHAR: CommandId = CommandId::new(MODULE, "execute-find-char");

// =============================================================================
// Visual Mode Manipulation
// =============================================================================

/// Swap cursor and anchor in visual mode (o).
pub const VISUAL_SWAP_ANCHOR: CommandId = CommandId::new(MODULE, "visual-swap-anchor");

/// Toggle to visual char mode.
pub const TOGGLE_VISUAL_CHAR: CommandId = CommandId::new(MODULE, "toggle-visual-char");

/// Toggle to visual line mode.
pub const TOGGLE_VISUAL_LINE: CommandId = CommandId::new(MODULE, "toggle-visual-line");

/// Toggle to visual block mode.
pub const TOGGLE_VISUAL_BLOCK: CommandId = CommandId::new(MODULE, "toggle-visual-block");

/// Reselect last visual selection (gv).
pub const RESELECT_LAST: CommandId = CommandId::new(MODULE, "reselect-last");

// =============================================================================
// Visual Mode Operators
// =============================================================================

/// Delete selection (d, x in visual).
pub const DELETE_SELECTION: CommandId = CommandId::new(MODULE, "delete-selection");

/// Yank selection (y in visual).
pub const YANK_SELECTION: CommandId = CommandId::new(MODULE, "yank-selection");

/// Change selection (c, s in visual).
pub const CHANGE_SELECTION: CommandId = CommandId::new(MODULE, "change-selection");

/// Indent selection (> in visual).
pub const INDENT_SELECTION: CommandId = CommandId::new(MODULE, "indent-selection");

/// Dedent selection (< in visual).
pub const DEDENT_SELECTION: CommandId = CommandId::new(MODULE, "dedent-selection");

// =============================================================================
// Change Operations (vim-specific, enters insert after)
// =============================================================================

/// Change line (cc, S).
pub const CHANGE_LINE: CommandId = CommandId::new(MODULE, "change-line");

/// Change to end of line (C).
pub const CHANGE_TO_EOL: CommandId = CommandId::new(MODULE, "change-to-eol");

// =============================================================================
// Dot Repeat (Epic #465)
// =============================================================================

/// Repeat last change (.).
///
/// Replays the last change operation (operator + motion/text object, or
/// insert mode edits). Count overrides the original count.
pub const DOT_REPEAT: CommandId = CommandId::new(MODULE, "dot-repeat");

// =============================================================================
// Macro Recording/Playback (Epic #465 Phase 8D)
// =============================================================================

/// Execute macro from register (@{a-z}).
///
/// Plays back a recorded macro from the specified register.
/// The register is provided in the resolve context metadata.
pub const PLAY_MACRO: CommandId = CommandId::new(MODULE, "play-macro");

/// Repeat last macro (@@).
///
/// Plays the same macro that was last played with @{a-z}.
pub const REPEAT_MACRO: CommandId = CommandId::new(MODULE, "repeat-macro");

// =============================================================================
// Session Management (not yet implemented)
// =============================================================================

/// Detach from server (server continues).
pub const SESSION_DETACH: CommandId = CommandId::new(MODULE, "session-detach");

/// List running server instances.
pub const SESSION_SERVERS: CommandId = CommandId::new(MODULE, "session-servers");

/// Kill the current server.
pub const SESSION_KILL_SERVER: CommandId = CommandId::new(MODULE, "session-kill-server");

// =============================================================================
// Visual Mode Additional Operations (not yet implemented)
// =============================================================================

/// Swap cursor to opposite corner in block mode.
pub const VISUAL_SWAP_CORNER: CommandId = CommandId::new(MODULE, "visual-swap-corner");

/// Toggle case of selection.
pub const TOGGLE_CASE_SELECTION: CommandId = CommandId::new(MODULE, "toggle-case-selection");

/// Lowercase selection.
pub const LOWERCASE_SELECTION: CommandId = CommandId::new(MODULE, "lowercase-selection");

/// Uppercase selection.
pub const UPPERCASE_SELECTION: CommandId = CommandId::new(MODULE, "uppercase-selection");

/// Join selected lines.
pub const JOIN_SELECTION: CommandId = CommandId::new(MODULE, "join-selection");

/// Enter command mode with selection range.
pub const COMMAND_WITH_SELECTION: CommandId = CommandId::new(MODULE, "command-with-selection");

/// No-op for blocked keys in visual mode.
pub const VISUAL_NOOP: CommandId = CommandId::new(MODULE, "visual-noop");

/// Exit visual, move to line start, enter insert.
pub const VISUAL_INSERT_START: CommandId = CommandId::new(MODULE, "visual-insert-start");

/// Exit visual, move to line end, enter insert.
pub const VISUAL_INSERT_END: CommandId = CommandId::new(MODULE, "visual-insert-end");

/// Insert at block left column on all lines.
pub const BLOCK_INSERT_START: CommandId = CommandId::new(MODULE, "block-insert-start");

/// Append at block right column on all lines.
pub const BLOCK_INSERT_END: CommandId = CommandId::new(MODULE, "block-insert-end");

// =============================================================================
// Text Objects (not yet implemented - will move to textobjects module)
// =============================================================================

/// Inner word text object.
pub const INNER_WORD: CommandId = CommandId::new(MODULE, "inner-word");
/// Inner WORD text object.
pub const INNER_WORD_BIG: CommandId = CommandId::new(MODULE, "inner-word-big");
/// Inner double quote text object.
pub const INNER_DOUBLE_QUOTE: CommandId = CommandId::new(MODULE, "inner-double-quote");
/// Inner single quote text object.
pub const INNER_SINGLE_QUOTE: CommandId = CommandId::new(MODULE, "inner-single-quote");
/// Inner backtick text object.
pub const INNER_BACKTICK: CommandId = CommandId::new(MODULE, "inner-backtick");
/// Inner parentheses text object.
pub const INNER_PAREN: CommandId = CommandId::new(MODULE, "inner-paren");
/// Inner brackets text object.
pub const INNER_BRACKET: CommandId = CommandId::new(MODULE, "inner-bracket");
/// Inner braces text object.
pub const INNER_BRACE: CommandId = CommandId::new(MODULE, "inner-brace");
/// Inner angle brackets text object.
pub const INNER_ANGLE: CommandId = CommandId::new(MODULE, "inner-angle");
/// Inner tag text object.
pub const INNER_TAG: CommandId = CommandId::new(MODULE, "inner-tag");
/// Inner sentence text object.
pub const INNER_SENTENCE: CommandId = CommandId::new(MODULE, "inner-sentence");
/// Inner paragraph text object.
pub const INNER_PARAGRAPH: CommandId = CommandId::new(MODULE, "inner-paragraph");

/// Around word text object.
pub const AROUND_WORD: CommandId = CommandId::new(MODULE, "around-word");
/// Around WORD text object.
pub const AROUND_WORD_BIG: CommandId = CommandId::new(MODULE, "around-word-big");
/// Around double quote text object.
pub const AROUND_DOUBLE_QUOTE: CommandId = CommandId::new(MODULE, "around-double-quote");
/// Around single quote text object.
pub const AROUND_SINGLE_QUOTE: CommandId = CommandId::new(MODULE, "around-single-quote");
/// Around backtick text object.
pub const AROUND_BACKTICK: CommandId = CommandId::new(MODULE, "around-backtick");
/// Around parentheses text object.
pub const AROUND_PAREN: CommandId = CommandId::new(MODULE, "around-paren");
/// Around brackets text object.
pub const AROUND_BRACKET: CommandId = CommandId::new(MODULE, "around-bracket");
/// Around braces text object.
pub const AROUND_BRACE: CommandId = CommandId::new(MODULE, "around-brace");
/// Around angle brackets text object.
pub const AROUND_ANGLE: CommandId = CommandId::new(MODULE, "around-angle");
/// Around tag text object.
pub const AROUND_TAG: CommandId = CommandId::new(MODULE, "around-tag");
/// Around sentence text object.
pub const AROUND_SENTENCE: CommandId = CommandId::new(MODULE, "around-sentence");
/// Around paragraph text object.
pub const AROUND_PARAGRAPH: CommandId = CommandId::new(MODULE, "around-paragraph");

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // OperatorId tests
    // ========================================================================

    #[test]
    fn test_operator_id_constants() {
        assert_eq!(DELETE.name(), "delete");
        assert_eq!(YANK.name(), "yank");
        assert_eq!(CHANGE.name(), "change");
    }

    #[test]
    fn test_operator_id_module() {
        assert_eq!(DELETE.module().as_str(), "vim");
        assert_eq!(YANK.module().as_str(), "vim");
        assert_eq!(CHANGE.module().as_str(), "vim");
    }

    #[test]
    fn test_operator_id_display() {
        assert_eq!(format!("{DELETE}"), "vim:delete");
        assert_eq!(format!("{YANK}"), "vim:yank");
        assert_eq!(format!("{CHANGE}"), "vim:change");
    }

    #[test]
    fn test_operator_id_equality() {
        let delete2 = OperatorId::new(MODULE, "delete");
        assert_eq!(DELETE, delete2);
        assert_ne!(DELETE, YANK);
    }

    #[test]
    fn test_operator_id_from_qualified_leaked() {
        let op = OperatorId::from_qualified_leaked("vim:delete".to_string());
        assert_eq!(op.module().as_str(), "vim");
        assert_eq!(op.name(), "delete");
    }

    #[test]
    fn test_operator_id_from_qualified_leaked_no_colon() {
        let op = OperatorId::from_qualified_leaked("delete".to_string());
        assert_eq!(op.module().as_str(), "unknown");
        assert_eq!(op.name(), "delete");
    }

    #[test]
    fn test_operator_id_hash() {
        use std::collections::HashSet;

        let op1 = OperatorId::new(MODULE, "delete");
        let op2 = OperatorId::new(MODULE, "delete");
        let op3 = OperatorId::new(MODULE, "yank");

        let mut set = HashSet::new();
        set.insert(op1);
        assert!(set.contains(&op2)); // Same name
        assert!(!set.contains(&op3)); // Different name
    }

    // ========================================================================
    // CommandId tests
    // ========================================================================

    #[test]
    fn test_command_id_constants() {
        assert_eq!(ENTER_INSERT.name(), "enter-insert");
        assert_eq!(EXIT_INSERT.name(), "exit-insert");
    }

    #[test]
    fn test_command_id_module() {
        assert_eq!(ENTER_INSERT.module().as_str(), "vim");
    }
}
