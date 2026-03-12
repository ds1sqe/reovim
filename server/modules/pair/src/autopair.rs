//! Auto-pair insertion logic.
//!
//! Context-aware: skips insertion inside string/comment nodes.

use reovim_driver_syntax::{BracketConfig, SyntaxContext};

/// Determine if auto-pair should fire for a character insertion.
///
/// Returns the closing character to insert, or `None` if auto-pair
/// should not fire (wrong context, char not in config, etc.).
#[must_use]
pub fn should_autopair(ch: char, config: &BracketConfig, context: SyntaxContext) -> Option<char> {
    // Don't auto-pair inside strings or comments
    if context != SyntaxContext::Code {
        return None;
    }

    // Find matching pair where ch is the opener
    config
        .autopair_pairs()
        .iter()
        .find(|p| p.open == ch)
        .map(|p| p.close)
}

#[cfg(test)]
#[path = "autopair_tests.rs"]
mod tests;
