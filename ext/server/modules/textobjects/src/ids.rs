//! Command ID constants for the textobjects module.
//!
//! These constants enable compile-time verification of command IDs
//! referenced in keybindings. Import these when defining keybindings
//! instead of using string literals.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Textobjects module ID.
pub const MODULE: ModuleId = ModuleId::new("textobjects");

// =============================================================================
// Inner Text Objects
// =============================================================================

/// Inner word text object (iw).
pub const INNER_WORD: CommandId = CommandId::new(MODULE, "inner-word");

/// Inner WORD text object (iW).
pub const INNER_WORD_BIG: CommandId = CommandId::new(MODULE, "inner-word-big");

/// Inner double quote text object (i").
pub const INNER_DOUBLE_QUOTE: CommandId = CommandId::new(MODULE, "inner-double-quote");

/// Inner single quote text object (i').
pub const INNER_SINGLE_QUOTE: CommandId = CommandId::new(MODULE, "inner-single-quote");

/// Inner backtick text object (i followed by backtick).
pub const INNER_BACKTICK: CommandId = CommandId::new(MODULE, "inner-backtick");

/// Inner parentheses text object (i(, i), ib).
pub const INNER_PAREN: CommandId = CommandId::new(MODULE, "inner-paren");

/// Inner brackets text object (i[, i]).
pub const INNER_BRACKET: CommandId = CommandId::new(MODULE, "inner-bracket");

/// Inner braces text object (i{, i}, iB).
pub const INNER_BRACE: CommandId = CommandId::new(MODULE, "inner-brace");

/// Inner angle brackets text object (i<, i>).
pub const INNER_ANGLE: CommandId = CommandId::new(MODULE, "inner-angle");

/// Inner tag text object (it).
pub const INNER_TAG: CommandId = CommandId::new(MODULE, "inner-tag");

/// Inner sentence text object (is).
pub const INNER_SENTENCE: CommandId = CommandId::new(MODULE, "inner-sentence");

/// Inner paragraph text object (ip).
pub const INNER_PARAGRAPH: CommandId = CommandId::new(MODULE, "inner-paragraph");

// =============================================================================
// Around Text Objects
// =============================================================================

/// Around word text object (aw).
pub const AROUND_WORD: CommandId = CommandId::new(MODULE, "around-word");

/// Around WORD text object (aW).
pub const AROUND_WORD_BIG: CommandId = CommandId::new(MODULE, "around-word-big");

/// Around double quote text object (a").
pub const AROUND_DOUBLE_QUOTE: CommandId = CommandId::new(MODULE, "around-double-quote");

/// Around single quote text object (a').
pub const AROUND_SINGLE_QUOTE: CommandId = CommandId::new(MODULE, "around-single-quote");

/// Around backtick text object (a followed by backtick).
pub const AROUND_BACKTICK: CommandId = CommandId::new(MODULE, "around-backtick");

/// Around parentheses text object (a(, a), ab).
pub const AROUND_PAREN: CommandId = CommandId::new(MODULE, "around-paren");

/// Around brackets text object (a[, a]).
pub const AROUND_BRACKET: CommandId = CommandId::new(MODULE, "around-bracket");

/// Around braces text object (a{, a}, aB).
pub const AROUND_BRACE: CommandId = CommandId::new(MODULE, "around-brace");

/// Around angle brackets text object (a<, a>).
pub const AROUND_ANGLE: CommandId = CommandId::new(MODULE, "around-angle");

/// Around tag text object (at).
pub const AROUND_TAG: CommandId = CommandId::new(MODULE, "around-tag");

/// Around sentence text object (as).
pub const AROUND_SENTENCE: CommandId = CommandId::new(MODULE, "around-sentence");

/// Around paragraph text object (ap).
pub const AROUND_PARAGRAPH: CommandId = CommandId::new(MODULE, "around-paragraph");

// =============================================================================
// Semantic Text Objects (treesitter-based)
// =============================================================================

/// Inner function text object (if).
pub const INNER_FUNCTION: CommandId = CommandId::new(MODULE, "inner-function");

/// Around function text object (af).
pub const AROUND_FUNCTION: CommandId = CommandId::new(MODULE, "around-function");

/// Inner class text object (ic).
pub const INNER_CLASS: CommandId = CommandId::new(MODULE, "inner-class");

/// Around class text object (ac).
pub const AROUND_CLASS: CommandId = CommandId::new(MODULE, "around-class");

/// Inner argument text object (ia).
pub const INNER_ARGUMENT: CommandId = CommandId::new(MODULE, "inner-argument");

/// Around argument text object (aa).
pub const AROUND_ARGUMENT: CommandId = CommandId::new(MODULE, "around-argument");

/// Inner conditional text object (io).
pub const INNER_CONDITIONAL: CommandId = CommandId::new(MODULE, "inner-conditional");

/// Around conditional text object (ao).
pub const AROUND_CONDITIONAL: CommandId = CommandId::new(MODULE, "around-conditional");

/// Inner loop text object (il).
pub const INNER_LOOP: CommandId = CommandId::new(MODULE, "inner-loop");

/// Around loop text object (al).
pub const AROUND_LOOP: CommandId = CommandId::new(MODULE, "around-loop");

/// Inner comment text object (i/).
pub const INNER_COMMENT: CommandId = CommandId::new(MODULE, "inner-comment");

/// Around comment text object (a/).
pub const AROUND_COMMENT: CommandId = CommandId::new(MODULE, "around-comment");

/// Inner block text object (treesitter-based, distinct from bracket iB).
pub const INNER_BLOCK_TS: CommandId = CommandId::new(MODULE, "inner-block-ts");

/// Around block text object (treesitter-based, distinct from bracket aB).
pub const AROUND_BLOCK_TS: CommandId = CommandId::new(MODULE, "around-block-ts");
