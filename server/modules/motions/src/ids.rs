//! Command ID constants for the motions module.
//!
//! These constants enable compile-time verification of command IDs
//! referenced in keybindings. Import these when defining keybindings
//! instead of using string literals.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Motions module ID.
pub const MODULE: ModuleId = ModuleId::new("motions");

// =============================================================================
// Word Motions
// =============================================================================

/// Move to next word (w).
pub const WORD_FORWARD: CommandId = CommandId::new(MODULE, "word-forward");

/// Move to previous word (b).
pub const WORD_BACKWARD: CommandId = CommandId::new(MODULE, "word-backward");

/// Move to end of word (e).
pub const WORD_END: CommandId = CommandId::new(MODULE, "word-end");

/// Move to next WORD (W).
pub const WORD_FORWARD_BIG: CommandId = CommandId::new(MODULE, "word-forward-big");

/// Move to previous WORD (B).
pub const WORD_BACKWARD_BIG: CommandId = CommandId::new(MODULE, "word-backward-big");

/// Move to end of WORD (E).
pub const WORD_END_BIG: CommandId = CommandId::new(MODULE, "word-end-big");

/// Move to end of previous word (ge).
pub const WORD_END_BACKWARD: CommandId = CommandId::new(MODULE, "word-end-backward");

/// Move to end of previous WORD (gE).
pub const WORD_END_BACKWARD_BIG: CommandId = CommandId::new(MODULE, "word-end-backward-big");

// =============================================================================
// Line Motions
// =============================================================================

/// Move to start of line (0).
pub const LINE_START: CommandId = CommandId::new(MODULE, "line-start");

/// Move to end of line ($).
pub const LINE_END: CommandId = CommandId::new(MODULE, "line-end");

/// Move to first non-blank character (^).
pub const FIRST_NON_BLANK: CommandId = CommandId::new(MODULE, "first-non-blank");

// =============================================================================
// Document Motions
// =============================================================================

/// Go to start of document (gg).
pub const DOCUMENT_START: CommandId = CommandId::new(MODULE, "document-start");

/// Go to end of document (G).
pub const DOCUMENT_END: CommandId = CommandId::new(MODULE, "document-end");

/// Select whole line (text object).
pub const WHOLE_LINE: CommandId = CommandId::new(MODULE, "whole-line");

// =============================================================================
// Find Character Motions
// =============================================================================

/// Coordinator command for find-char motions (#563).
///
/// WARNING: Do NOT override this command. It records `FindCharState` for
/// repeat (`;`/`,`) and then delegates to `EXECUTE_FIND_CHAR`. Overriding
/// this would break repeat recording for all providers. Override
/// `EXECUTE_FIND_CHAR` instead.
pub const DISPATCH_FIND_CHAR: CommandId = CommandId::new(MODULE, "dispatch-find-char");

/// Find character forward (f).
pub const FIND_CHAR_FORWARD: CommandId = CommandId::new(MODULE, "find-char-forward");

/// Find character backward (F).
pub const FIND_CHAR_BACKWARD: CommandId = CommandId::new(MODULE, "find-char-backward");

/// Find character forward, stop before (t).
pub const TILL_CHAR_FORWARD: CommandId = CommandId::new(MODULE, "till-char-forward");

/// Find character backward, stop after (T).
pub const TILL_CHAR_BACKWARD: CommandId = CommandId::new(MODULE, "till-char-backward");

/// Repeat find character same direction (;).
pub const REPEAT_FIND_SAME: CommandId = CommandId::new(MODULE, "repeat-find-same");

/// Repeat find character reverse direction (,).
pub const REPEAT_FIND_REVERSE: CommandId = CommandId::new(MODULE, "repeat-find-reverse");

// =============================================================================
// Search Motions
// =============================================================================

/// Search forward (/).
pub const SEARCH_FORWARD: CommandId = CommandId::new(MODULE, "search-forward");

/// Search backward (?).
pub const SEARCH_BACKWARD: CommandId = CommandId::new(MODULE, "search-backward");

/// Next search result (n).
pub const SEARCH_NEXT: CommandId = CommandId::new(MODULE, "search-next");

/// Previous search result (N).
pub const SEARCH_PREV: CommandId = CommandId::new(MODULE, "search-prev");

/// Search word under cursor forward (*).
pub const SEARCH_WORD_FORWARD: CommandId = CommandId::new(MODULE, "search-word-forward");

/// Search word under cursor backward (#).
pub const SEARCH_WORD_BACKWARD: CommandId = CommandId::new(MODULE, "search-word-backward");

/// Clear search highlight.
pub const CLEAR_SEARCH_HIGHLIGHT: CommandId = CommandId::new(MODULE, "clear-search-highlight");
