//! Command ID constants for the vim module.
//!
//! The vim module defines mode-switching commands and vim-specific behavior.
//! These constants enable compile-time verification of command IDs
//! referenced in keybindings.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Vim module ID.
pub const MODULE: ModuleId = ModuleId::new("vim");

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

/// Exit command-line mode (Esc).
pub const EXIT_COMMANDLINE: CommandId = CommandId::new(MODULE, "exit-commandline");

/// Enter window mode (Ctrl-w).
pub const ENTER_WINDOW_MODE: CommandId = CommandId::new(MODULE, "enter-window-mode");

/// Exit operator-pending mode (Esc).
pub const EXIT_OPERATOR_PENDING: CommandId = CommandId::new(MODULE, "exit-operator-pending");

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
