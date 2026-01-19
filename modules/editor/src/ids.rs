//! Command ID constants for the editor module.
//!
//! These constants enable compile-time verification of command IDs
//! referenced in keybindings. Import these when defining keybindings
//! instead of using string literals.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Editor module ID.
pub const MODULE: ModuleId = ModuleId::new("editor");

// =============================================================================
// Cursor Movement
// =============================================================================

/// Move cursor up (k).
pub const CURSOR_UP: CommandId = CommandId::new(MODULE, "cursor-up");

/// Move cursor down (j).
pub const CURSOR_DOWN: CommandId = CommandId::new(MODULE, "cursor-down");

/// Move cursor left (h).
pub const CURSOR_LEFT: CommandId = CommandId::new(MODULE, "cursor-left");

/// Move cursor right (l).
pub const CURSOR_RIGHT: CommandId = CommandId::new(MODULE, "cursor-right");

/// Move cursor down by display line (gj).
pub const CURSOR_DISPLAY_DOWN: CommandId = CommandId::new(MODULE, "cursor-display-down");

/// Move cursor up by display line (gk).
pub const CURSOR_DISPLAY_UP: CommandId = CommandId::new(MODULE, "cursor-display-up");

// =============================================================================
// Insert Mode Edits
// =============================================================================

/// Insert newline.
pub const INSERT_NEWLINE: CommandId = CommandId::new(MODULE, "insert-newline");

/// Insert tab.
pub const INSERT_TAB: CommandId = CommandId::new(MODULE, "insert-tab");

/// Delete word before cursor (Ctrl-w in insert).
pub const DELETE_WORD_BEFORE: CommandId = CommandId::new(MODULE, "delete-word-before");

/// Delete to beginning of line (Ctrl-u in insert).
pub const DELETE_TO_BOL: CommandId = CommandId::new(MODULE, "delete-to-bol");

// =============================================================================
// Line Navigation (for insert mode Home/End)
// =============================================================================

/// Move to start of line (Home).
pub const LINE_START: CommandId = CommandId::new(MODULE, "line-start");

/// Move to end of line (End).
pub const LINE_END: CommandId = CommandId::new(MODULE, "line-end");

// =============================================================================
// Completion
// =============================================================================

/// Next completion item (Ctrl-n).
pub const COMPLETION_NEXT: CommandId = CommandId::new(MODULE, "completion-next");

/// Previous completion item (Ctrl-p).
pub const COMPLETION_PREV: CommandId = CommandId::new(MODULE, "completion-prev");

/// Trigger completion (Ctrl-Space).
pub const COMPLETION_TRIGGER: CommandId = CommandId::new(MODULE, "completion-trigger");

// =============================================================================
// Delete Operations
// =============================================================================

/// Delete character under cursor (x).
pub const DELETE_CHAR: CommandId = CommandId::new(MODULE, "delete-char");

/// Delete character before cursor (X).
pub const DELETE_CHAR_BEFORE: CommandId = CommandId::new(MODULE, "delete-char-before");

/// Delete line (dd).
pub const DELETE_LINE: CommandId = CommandId::new(MODULE, "delete-line");

/// Delete to end of line (D).
pub const DELETE_TO_EOL: CommandId = CommandId::new(MODULE, "delete-to-eol");

// =============================================================================
// Operators
// =============================================================================

/// Enter delete operator mode (d).
pub const ENTER_DELETE_OPERATOR: CommandId = CommandId::new(MODULE, "enter-delete-operator");

/// Enter yank operator mode (y).
pub const ENTER_YANK_OPERATOR: CommandId = CommandId::new(MODULE, "enter-yank-operator");

/// Enter change operator mode (c).
pub const ENTER_CHANGE_OPERATOR: CommandId = CommandId::new(MODULE, "enter-change-operator");

/// Enter indent operator mode (>).
pub const ENTER_INDENT_OPERATOR: CommandId = CommandId::new(MODULE, "enter-indent-operator");

/// Enter dedent operator mode (<).
pub const ENTER_DEDENT_OPERATOR: CommandId = CommandId::new(MODULE, "enter-dedent-operator");

// =============================================================================
// Yank/Paste
// =============================================================================

/// Yank line (yy, Y).
pub const YANK_LINE: CommandId = CommandId::new(MODULE, "yank-line");

/// Paste after cursor (p).
pub const PASTE_AFTER: CommandId = CommandId::new(MODULE, "paste-after");

/// Paste before cursor (P).
pub const PASTE_BEFORE: CommandId = CommandId::new(MODULE, "paste-before");

// =============================================================================
// Undo/Redo
// =============================================================================

/// Undo (u).
pub const UNDO: CommandId = CommandId::new(MODULE, "undo");

/// Redo (ctrl-r).
pub const REDO: CommandId = CommandId::new(MODULE, "redo");

// =============================================================================
// Replace/Repeat
// =============================================================================

/// Start replace character (r).
pub const REPLACE_CHAR_START: CommandId = CommandId::new(MODULE, "replace-char-start");

/// Repeat last change (.).
pub const REPEAT_DOT: CommandId = CommandId::new(MODULE, "repeat-dot");

/// Join lines (J).
pub const JOIN_LINES: CommandId = CommandId::new(MODULE, "join-lines");

// =============================================================================
// File Operations
// =============================================================================

/// Write buffer to file (:w).
pub const WRITE: CommandId = CommandId::new(MODULE, "write");

// =============================================================================
// Scroll Operations (not yet implemented)
// =============================================================================

/// Scroll half page up (Ctrl-u).
pub const SCROLL_HALF_UP: CommandId = CommandId::new(MODULE, "scroll-half-up");

/// Scroll half page down (Ctrl-d).
pub const SCROLL_HALF_DOWN: CommandId = CommandId::new(MODULE, "scroll-half-down");

/// Scroll page up (Ctrl-b).
pub const SCROLL_PAGE_UP: CommandId = CommandId::new(MODULE, "scroll-page-up");

/// Scroll page down (Ctrl-f).
pub const SCROLL_PAGE_DOWN: CommandId = CommandId::new(MODULE, "scroll-page-down");

/// Center cursor line (zz).
pub const SCROLL_CENTER: CommandId = CommandId::new(MODULE, "scroll-center");

/// Scroll cursor line to top (zt).
pub const SCROLL_TOP: CommandId = CommandId::new(MODULE, "scroll-top");

/// Scroll cursor line to bottom (zb).
pub const SCROLL_BOTTOM: CommandId = CommandId::new(MODULE, "scroll-bottom");

// =============================================================================
// Mark Operations (not yet implemented)
// =============================================================================

/// Set mark (m).
pub const SET_MARK: CommandId = CommandId::new(MODULE, "set-mark");

/// Go to mark line (').
pub const GOTO_MARK_LINE: CommandId = CommandId::new(MODULE, "goto-mark-line");

/// Go to mark exact position (backtick).
pub const GOTO_MARK_EXACT: CommandId = CommandId::new(MODULE, "goto-mark-exact");

// =============================================================================
// Case Operations (not yet implemented)
// =============================================================================

/// Replace character (r).
pub const REPLACE_CHAR: CommandId = CommandId::new(MODULE, "replace-char");

/// Toggle case (~).
pub const TOGGLE_CASE: CommandId = CommandId::new(MODULE, "toggle-case");
