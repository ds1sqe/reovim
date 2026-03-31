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

use std::{borrow::Cow, fmt};

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
    name: Cow<'static, str>,
}

impl OperatorId {
    /// Create a new operator identifier from static strings.
    #[must_use]
    pub const fn new(module: ModuleId, name: &'static str) -> Self {
        Self {
            module,
            name: Cow::Borrowed(name),
        }
    }

    /// Create an operator identifier from a qualified string like "module:operator".
    ///
    /// This method is intended for dynamic use cases like FFI where operator IDs
    /// are specified as strings at runtime.
    ///
    /// If the string doesn't contain ':', the entire string is treated as the
    /// operator name with "unknown" as the module.
    #[must_use]
    pub fn from_qualified(qualified: String) -> Self {
        let (module_str, name_str) = if let Some(idx) = qualified.find(':') {
            (qualified[..idx].to_string(), qualified[idx + 1..].to_string())
        } else {
            ("unknown".to_string(), qualified)
        };

        Self {
            module: ModuleId::from_string(module_str),
            name: Cow::Owned(name_str),
        }
    }

    /// Get the owning module.
    #[must_use]
    pub const fn module(&self) -> &ModuleId {
        &self.module
    }

    /// Get the local name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the local name as an owned `Cow`.
    #[must_use]
    pub fn name_owned(&self) -> Cow<'static, str> {
        self.name.clone()
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

/// Lowercase operator - converts text to lowercase.
///
/// Vim equivalent: `gu`
pub const LOWERCASE: OperatorId = OperatorId::new(MODULE, "lowercase");

/// Uppercase operator - converts text to uppercase.
///
/// Vim equivalent: `gU`
pub const UPPERCASE: OperatorId = OperatorId::new(MODULE, "uppercase");

/// Toggle case operator - swaps uppercase/lowercase.
///
/// Vim equivalent: `g~`
pub const TOGGLE_CASE_OP: OperatorId = OperatorId::new(MODULE, "toggle-case");

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

// Command-line editing commands (#451)

/// Move cursor left in command-line.
pub const CMDLINE_CURSOR_LEFT: CommandId = CommandId::new(MODULE, "cmdline-cursor-left");
/// Move cursor right in command-line.
pub const CMDLINE_CURSOR_RIGHT: CommandId = CommandId::new(MODULE, "cmdline-cursor-right");
/// Move cursor to start of command-line.
pub const CMDLINE_CURSOR_HOME: CommandId = CommandId::new(MODULE, "cmdline-cursor-home");
/// Move cursor to end of command-line.
pub const CMDLINE_CURSOR_END: CommandId = CommandId::new(MODULE, "cmdline-cursor-end");
/// Delete character at cursor in command-line.
pub const CMDLINE_DELETE_CHAR: CommandId = CommandId::new(MODULE, "cmdline-delete-char");
/// Delete character before cursor in command-line.
pub const CMDLINE_BACKSPACE: CommandId = CommandId::new(MODULE, "cmdline-backspace");
/// Delete word before cursor in command-line.
pub const CMDLINE_DELETE_WORD: CommandId = CommandId::new(MODULE, "cmdline-delete-word");
/// Delete to start of command-line.
pub const CMDLINE_DELETE_TO_START: CommandId = CommandId::new(MODULE, "cmdline-delete-to-start");
/// Navigate to older history entry.
pub const CMDLINE_HISTORY_UP: CommandId = CommandId::new(MODULE, "cmdline-history-up");
/// Navigate to newer history entry.
pub const CMDLINE_HISTORY_DOWN: CommandId = CommandId::new(MODULE, "cmdline-history-down");
/// Cycle to next completion.
pub const CMDLINE_COMPLETE_NEXT: CommandId = CommandId::new(MODULE, "cmdline-complete-next");
/// Cycle to previous completion.
pub const CMDLINE_COMPLETE_PREV: CommandId = CommandId::new(MODULE, "cmdline-complete-prev");

/// Enter window mode (Ctrl-w).
pub const ENTER_WINDOW_MODE: CommandId = CommandId::new(MODULE, "enter-window-mode");

/// Enter replace mode (R).
///
/// Switches to replace mode where typed characters overwrite existing text.
/// Backspace restores original characters.
pub const ENTER_REPLACE_MODE: CommandId = CommandId::new(MODULE, "enter-replace-mode");

/// Backspace in replace mode.
///
/// Restores the original character that was overwritten by replace mode.
/// Pops from the replace restore stack in `VimSessionState`.
pub const REPLACE_BACKSPACE: CommandId = CommandId::new(MODULE, "replace-backspace");

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
/// - `find_inclusive`: true for f/F, false for t/T (`ArgValue::Bang`)
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

    // === Coverage: lines 107-109 — OperatorId::name_owned() ===

    #[test]
    fn operator_id_name_owned_static() {
        // Constant uses Cow::Borrowed — name_owned() clones the Cow.
        let owned = DELETE.name_owned();
        assert_eq!(owned, "delete");
    }

    #[test]
    fn operator_id_name_owned_dynamic() {
        // from_qualified uses Cow::Owned — name_owned() clones the Cow.
        let op = OperatorId::from_qualified("mymod:custom-op".to_string());
        let owned = op.name_owned();
        assert_eq!(owned, "custom-op");
    }
}
