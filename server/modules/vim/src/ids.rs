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

    // ========================================================================
    // Insert mode command IDs
    // ========================================================================

    #[test]
    fn test_insert_mode_command_ids() {
        assert_eq!(ENTER_INSERT.name(), "enter-insert");
        assert_eq!(ENTER_INSERT_AFTER.name(), "enter-insert-after");
        assert_eq!(ENTER_INSERT_EOL.name(), "enter-insert-eol");
        assert_eq!(ENTER_INSERT_BOL.name(), "enter-insert-bol");
        assert_eq!(OPEN_LINE_BELOW.name(), "open-line-below");
        assert_eq!(OPEN_LINE_ABOVE.name(), "open-line-above");
        assert_eq!(EXIT_INSERT.name(), "exit-insert");
    }

    #[test]
    fn test_insert_mode_command_ids_module() {
        assert_eq!(ENTER_INSERT_AFTER.module().as_str(), "vim");
        assert_eq!(ENTER_INSERT_EOL.module().as_str(), "vim");
        assert_eq!(ENTER_INSERT_BOL.module().as_str(), "vim");
        assert_eq!(OPEN_LINE_BELOW.module().as_str(), "vim");
        assert_eq!(OPEN_LINE_ABOVE.module().as_str(), "vim");
    }

    // ========================================================================
    // Visual mode command IDs
    // ========================================================================

    #[test]
    fn test_visual_mode_command_ids() {
        assert_eq!(ENTER_VISUAL.name(), "enter-visual");
        assert_eq!(ENTER_VISUAL_LINE.name(), "enter-visual-line");
        assert_eq!(ENTER_VISUAL_BLOCK.name(), "enter-visual-block");
        assert_eq!(EXIT_VISUAL.name(), "exit-visual");
    }

    #[test]
    fn test_visual_mode_command_ids_module() {
        assert_eq!(ENTER_VISUAL.module().as_str(), "vim");
        assert_eq!(ENTER_VISUAL_LINE.module().as_str(), "vim");
        assert_eq!(ENTER_VISUAL_BLOCK.module().as_str(), "vim");
        assert_eq!(EXIT_VISUAL.module().as_str(), "vim");
    }

    // ========================================================================
    // Other mode command IDs
    // ========================================================================

    #[test]
    fn test_other_mode_command_ids() {
        assert_eq!(ENTER_COMMANDLINE.name(), "enter-commandline");
        assert_eq!(CANCEL_COMMANDLINE.name(), "cancel-commandline");
        assert_eq!(EXIT_COMMANDLINE.name(), "exit-commandline");
        assert_eq!(ENTER_WINDOW_MODE.name(), "enter-window-mode");
        assert_eq!(CANCEL_TO_NORMAL.name(), "cancel-to-normal");
    }

    #[test]
    fn test_other_mode_command_ids_module() {
        assert_eq!(ENTER_COMMANDLINE.module().as_str(), "vim");
        assert_eq!(CANCEL_COMMANDLINE.module().as_str(), "vim");
        assert_eq!(EXIT_COMMANDLINE.module().as_str(), "vim");
        assert_eq!(ENTER_WINDOW_MODE.module().as_str(), "vim");
        assert_eq!(CANCEL_TO_NORMAL.module().as_str(), "vim");
    }

    // ========================================================================
    // Search command IDs
    // ========================================================================

    #[test]
    fn test_search_command_ids() {
        assert_eq!(ENTER_SEARCH_FORWARD.name(), "enter-search-forward");
        assert_eq!(ENTER_SEARCH_BACKWARD.name(), "enter-search-backward");
    }

    #[test]
    fn test_search_command_ids_module() {
        assert_eq!(ENTER_SEARCH_FORWARD.module().as_str(), "vim");
        assert_eq!(ENTER_SEARCH_BACKWARD.module().as_str(), "vim");
    }

    // ========================================================================
    // Find-char command ID
    // ========================================================================

    #[test]
    fn test_find_char_command_id() {
        assert_eq!(EXECUTE_FIND_CHAR.name(), "execute-find-char");
        assert_eq!(EXECUTE_FIND_CHAR.module().as_str(), "vim");
    }

    // ========================================================================
    // Visual manipulation command IDs
    // ========================================================================

    #[test]
    fn test_visual_manipulation_command_ids() {
        assert_eq!(VISUAL_SWAP_ANCHOR.name(), "visual-swap-anchor");
        assert_eq!(TOGGLE_VISUAL_CHAR.name(), "toggle-visual-char");
        assert_eq!(TOGGLE_VISUAL_LINE.name(), "toggle-visual-line");
        assert_eq!(TOGGLE_VISUAL_BLOCK.name(), "toggle-visual-block");
        assert_eq!(RESELECT_LAST.name(), "reselect-last");
    }

    // ========================================================================
    // Visual operator command IDs
    // ========================================================================

    #[test]
    fn test_visual_operator_command_ids() {
        assert_eq!(DELETE_SELECTION.name(), "delete-selection");
        assert_eq!(YANK_SELECTION.name(), "yank-selection");
        assert_eq!(CHANGE_SELECTION.name(), "change-selection");
        assert_eq!(INDENT_SELECTION.name(), "indent-selection");
        assert_eq!(DEDENT_SELECTION.name(), "dedent-selection");
    }

    // ========================================================================
    // Change operation command IDs
    // ========================================================================

    #[test]
    fn test_change_operation_command_ids() {
        assert_eq!(CHANGE_LINE.name(), "change-line");
        assert_eq!(CHANGE_TO_EOL.name(), "change-to-eol");
    }

    #[test]
    fn test_change_operation_command_ids_module() {
        assert_eq!(CHANGE_LINE.module().as_str(), "vim");
        assert_eq!(CHANGE_TO_EOL.module().as_str(), "vim");
    }

    // ========================================================================
    // Dot repeat and macro command IDs
    // ========================================================================

    #[test]
    fn test_dot_repeat_command_id() {
        assert_eq!(DOT_REPEAT.name(), "dot-repeat");
        assert_eq!(DOT_REPEAT.module().as_str(), "vim");
    }

    #[test]
    fn test_macro_command_ids() {
        assert_eq!(PLAY_MACRO.name(), "play-macro");
        assert_eq!(REPEAT_MACRO.name(), "repeat-macro");
        assert_eq!(PLAY_MACRO.module().as_str(), "vim");
        assert_eq!(REPEAT_MACRO.module().as_str(), "vim");
    }

    // ========================================================================
    // Session management command IDs
    // ========================================================================

    #[test]
    fn test_session_command_ids() {
        assert_eq!(SESSION_DETACH.name(), "session-detach");
        assert_eq!(SESSION_SERVERS.name(), "session-servers");
        assert_eq!(SESSION_KILL_SERVER.name(), "session-kill-server");
    }

    // ========================================================================
    // Visual additional operation command IDs
    // ========================================================================

    #[test]
    fn test_visual_additional_command_ids() {
        assert_eq!(VISUAL_SWAP_CORNER.name(), "visual-swap-corner");
        assert_eq!(TOGGLE_CASE_SELECTION.name(), "toggle-case-selection");
        assert_eq!(LOWERCASE_SELECTION.name(), "lowercase-selection");
        assert_eq!(UPPERCASE_SELECTION.name(), "uppercase-selection");
        assert_eq!(JOIN_SELECTION.name(), "join-selection");
        assert_eq!(COMMAND_WITH_SELECTION.name(), "command-with-selection");
        assert_eq!(VISUAL_NOOP.name(), "visual-noop");
        assert_eq!(VISUAL_INSERT_START.name(), "visual-insert-start");
        assert_eq!(VISUAL_INSERT_END.name(), "visual-insert-end");
        assert_eq!(BLOCK_INSERT_START.name(), "block-insert-start");
        assert_eq!(BLOCK_INSERT_END.name(), "block-insert-end");
    }

    // ========================================================================
    // Text object command IDs
    // ========================================================================

    #[test]
    fn test_inner_text_object_command_ids() {
        assert_eq!(INNER_WORD.name(), "inner-word");
        assert_eq!(INNER_WORD_BIG.name(), "inner-word-big");
        assert_eq!(INNER_DOUBLE_QUOTE.name(), "inner-double-quote");
        assert_eq!(INNER_SINGLE_QUOTE.name(), "inner-single-quote");
        assert_eq!(INNER_BACKTICK.name(), "inner-backtick");
        assert_eq!(INNER_PAREN.name(), "inner-paren");
        assert_eq!(INNER_BRACKET.name(), "inner-bracket");
        assert_eq!(INNER_BRACE.name(), "inner-brace");
        assert_eq!(INNER_ANGLE.name(), "inner-angle");
        assert_eq!(INNER_TAG.name(), "inner-tag");
        assert_eq!(INNER_SENTENCE.name(), "inner-sentence");
        assert_eq!(INNER_PARAGRAPH.name(), "inner-paragraph");
    }

    #[test]
    fn test_around_text_object_command_ids() {
        assert_eq!(AROUND_WORD.name(), "around-word");
        assert_eq!(AROUND_WORD_BIG.name(), "around-word-big");
        assert_eq!(AROUND_DOUBLE_QUOTE.name(), "around-double-quote");
        assert_eq!(AROUND_SINGLE_QUOTE.name(), "around-single-quote");
        assert_eq!(AROUND_BACKTICK.name(), "around-backtick");
        assert_eq!(AROUND_PAREN.name(), "around-paren");
        assert_eq!(AROUND_BRACKET.name(), "around-bracket");
        assert_eq!(AROUND_BRACE.name(), "around-brace");
        assert_eq!(AROUND_ANGLE.name(), "around-angle");
        assert_eq!(AROUND_TAG.name(), "around-tag");
        assert_eq!(AROUND_SENTENCE.name(), "around-sentence");
        assert_eq!(AROUND_PARAGRAPH.name(), "around-paragraph");
    }

    #[test]
    fn test_text_object_command_ids_all_vim_module() {
        assert_eq!(INNER_WORD.module().as_str(), "vim");
        assert_eq!(AROUND_WORD.module().as_str(), "vim");
        assert_eq!(INNER_PAREN.module().as_str(), "vim");
        assert_eq!(AROUND_PAREN.module().as_str(), "vim");
    }

    // ========================================================================
    // Module ID test
    // ========================================================================

    #[test]
    fn test_module_constant() {
        assert_eq!(MODULE.as_str(), "vim");
    }

    // ========================================================================
    // OperatorId additional tests
    // ========================================================================

    #[test]
    fn test_operator_id_debug() {
        let debug_str = format!("{DELETE:?}");
        assert!(debug_str.contains("OperatorId"));
        assert!(debug_str.contains("delete"));
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    fn test_operator_id_clone() {
        let cloned = DELETE.clone();
        assert_eq!(cloned, DELETE);
    }

    // ========================================================================
    // Additional OperatorId tests
    // ========================================================================

    #[test]
    fn test_operator_id_all_different() {
        assert_ne!(DELETE, YANK);
        assert_ne!(DELETE, CHANGE);
        assert_ne!(YANK, CHANGE);
    }

    #[test]
    fn test_operator_id_from_qualified_leaked_different_module() {
        let op = OperatorId::from_qualified_leaked("custom:my-op".to_string());
        assert_eq!(op.module().as_str(), "custom");
        assert_eq!(op.name(), "my-op");
    }

    #[test]
    fn test_operator_id_from_qualified_leaked_empty_name() {
        let op = OperatorId::from_qualified_leaked("vim:".to_string());
        assert_eq!(op.module().as_str(), "vim");
        assert_eq!(op.name(), "");
    }

    #[test]
    fn test_operator_id_from_qualified_leaked_colon_only() {
        let op = OperatorId::from_qualified_leaked(":".to_string());
        assert_eq!(op.module().as_str(), "");
        assert_eq!(op.name(), "");
    }

    #[test]
    fn test_operator_id_new_const() {
        const OP: OperatorId = OperatorId::new(MODULE, "test");
        assert_eq!(OP.name(), "test");
        assert_eq!(OP.module().as_str(), "vim");
    }

    #[test]
    fn test_operator_id_display_format() {
        let op = OperatorId::new(MODULE, "custom-op");
        assert_eq!(format!("{op}"), "vim:custom-op");
    }

    // ========================================================================
    // All command IDs have vim module
    // ========================================================================

    #[test]
    fn test_all_visual_manipulation_ids_vim_module() {
        assert_eq!(VISUAL_SWAP_ANCHOR.module().as_str(), "vim");
        assert_eq!(TOGGLE_VISUAL_CHAR.module().as_str(), "vim");
        assert_eq!(TOGGLE_VISUAL_LINE.module().as_str(), "vim");
        assert_eq!(TOGGLE_VISUAL_BLOCK.module().as_str(), "vim");
        assert_eq!(RESELECT_LAST.module().as_str(), "vim");
    }

    #[test]
    fn test_all_visual_operator_ids_vim_module() {
        assert_eq!(DELETE_SELECTION.module().as_str(), "vim");
        assert_eq!(YANK_SELECTION.module().as_str(), "vim");
        assert_eq!(CHANGE_SELECTION.module().as_str(), "vim");
        assert_eq!(INDENT_SELECTION.module().as_str(), "vim");
        assert_eq!(DEDENT_SELECTION.module().as_str(), "vim");
    }

    #[test]
    fn test_session_command_ids_vim_module() {
        assert_eq!(SESSION_DETACH.module().as_str(), "vim");
        assert_eq!(SESSION_SERVERS.module().as_str(), "vim");
        assert_eq!(SESSION_KILL_SERVER.module().as_str(), "vim");
    }

    #[test]
    fn test_visual_additional_ids_vim_module() {
        assert_eq!(VISUAL_SWAP_CORNER.module().as_str(), "vim");
        assert_eq!(TOGGLE_CASE_SELECTION.module().as_str(), "vim");
        assert_eq!(LOWERCASE_SELECTION.module().as_str(), "vim");
        assert_eq!(UPPERCASE_SELECTION.module().as_str(), "vim");
        assert_eq!(JOIN_SELECTION.module().as_str(), "vim");
        assert_eq!(COMMAND_WITH_SELECTION.module().as_str(), "vim");
        assert_eq!(VISUAL_NOOP.module().as_str(), "vim");
        assert_eq!(VISUAL_INSERT_START.module().as_str(), "vim");
        assert_eq!(VISUAL_INSERT_END.module().as_str(), "vim");
        assert_eq!(BLOCK_INSERT_START.module().as_str(), "vim");
        assert_eq!(BLOCK_INSERT_END.module().as_str(), "vim");
    }

    #[test]
    fn test_inner_text_object_ids_vim_module() {
        assert_eq!(INNER_WORD.module().as_str(), "vim");
        assert_eq!(INNER_WORD_BIG.module().as_str(), "vim");
        assert_eq!(INNER_DOUBLE_QUOTE.module().as_str(), "vim");
        assert_eq!(INNER_SINGLE_QUOTE.module().as_str(), "vim");
        assert_eq!(INNER_BACKTICK.module().as_str(), "vim");
        assert_eq!(INNER_PAREN.module().as_str(), "vim");
        assert_eq!(INNER_BRACKET.module().as_str(), "vim");
        assert_eq!(INNER_BRACE.module().as_str(), "vim");
        assert_eq!(INNER_ANGLE.module().as_str(), "vim");
        assert_eq!(INNER_TAG.module().as_str(), "vim");
        assert_eq!(INNER_SENTENCE.module().as_str(), "vim");
        assert_eq!(INNER_PARAGRAPH.module().as_str(), "vim");
    }

    #[test]
    fn test_around_text_object_ids_vim_module() {
        assert_eq!(AROUND_WORD.module().as_str(), "vim");
        assert_eq!(AROUND_WORD_BIG.module().as_str(), "vim");
        assert_eq!(AROUND_DOUBLE_QUOTE.module().as_str(), "vim");
        assert_eq!(AROUND_SINGLE_QUOTE.module().as_str(), "vim");
        assert_eq!(AROUND_BACKTICK.module().as_str(), "vim");
        assert_eq!(AROUND_PAREN.module().as_str(), "vim");
        assert_eq!(AROUND_BRACKET.module().as_str(), "vim");
        assert_eq!(AROUND_BRACE.module().as_str(), "vim");
        assert_eq!(AROUND_ANGLE.module().as_str(), "vim");
        assert_eq!(AROUND_TAG.module().as_str(), "vim");
        assert_eq!(AROUND_SENTENCE.module().as_str(), "vim");
        assert_eq!(AROUND_PARAGRAPH.module().as_str(), "vim");
    }

    #[test]
    fn test_macro_command_ids_names() {
        assert_eq!(PLAY_MACRO.name(), "play-macro");
        assert_eq!(REPEAT_MACRO.name(), "repeat-macro");
    }
}
