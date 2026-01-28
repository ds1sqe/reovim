//! Decoration generation for rainbow bracket highlighting.
//!
//! This module provides bracket decorations for the rendering system:
//! - Rainbow colors based on nesting depth
//! - Bold/underline for matched pair under cursor
//! - Red warning style for unmatched brackets
//!
//! # Rainbow Palette
//!
//! Uses a 6-color cycle that repeats for deeply nested brackets:
//! - Depth 0: Red
//! - Depth 1: Orange
//! - Depth 2: Yellow
//! - Depth 3: Green
//! - Depth 4: Blue
//! - Depth 5: Purple
//! - Depth 6+: Repeats from red

use {
    reovim_arch::Color,
    reovim_driver_display::{Decoration, DecorationGroup, Span, Style},
};

use crate::state::{BracketInfo, MatchedPair, SharedPairState};

/// Rainbow bracket colors (6-color palette, cycles for deeper nesting).
///
/// These are vibrant, high-contrast colors for visibility in both
/// dark and light themes. Theme integration (Phase 5) will allow
/// customization via `rainbow.bracket.{1-6}` highlight groups.
pub const RAINBOW_COLORS: [Color; 6] = [
    Color::Rgb {
        r: 229,
        g: 57,
        b: 53,
    }, // Red
    Color::Rgb {
        r: 255,
        g: 152,
        b: 0,
    }, // Orange
    Color::Rgb {
        r: 253,
        g: 216,
        b: 53,
    }, // Yellow
    Color::Rgb {
        r: 76,
        g: 175,
        b: 80,
    }, // Green
    Color::Rgb {
        r: 33,
        g: 150,
        b: 243,
    }, // Blue
    Color::Rgb {
        r: 156,
        g: 39,
        b: 176,
    }, // Purple
];

/// Warning color for unmatched brackets.
pub const UNMATCHED_COLOR: Color = Color::Red;

/// Get the rainbow color for a bracket depth.
///
/// Cycles through the 6-color palette for depths >= 6.
#[must_use]
pub const fn color_for_depth(depth: usize) -> Color {
    RAINBOW_COLORS[depth % RAINBOW_COLORS.len()]
}

/// Generate a style for a bracket at the given depth.
///
/// - Normal brackets: foreground color only
/// - Unmatched brackets (depth == `usize::MAX`): red + underline
#[must_use]
pub fn style_for_bracket(depth: usize) -> Style {
    if depth == usize::MAX {
        // Unmatched bracket: red with underline for warning
        Style::new().fg(UNMATCHED_COLOR).underline()
    } else {
        // Normal bracket: rainbow color based on depth
        Style::new().fg(color_for_depth(depth))
    }
}

/// Generate a style for a matched pair bracket (cursor is on/between them).
///
/// Uses the rainbow color plus bold and underline to make them distinctive.
#[must_use]
pub fn style_for_matched_pair(depth: usize) -> Style {
    if depth == usize::MAX {
        // Edge case: unmatched bracket shouldn't be in a matched pair,
        // but handle gracefully
        style_for_bracket(depth)
    } else {
        Style::new().fg(color_for_depth(depth)).bold().underline()
    }
}

/// Generate decorations for all brackets in a buffer.
///
/// Returns a vector of `Decoration::InlineStyle` items for each bracket,
/// using rainbow colors based on depth. Matched pair brackets (if any)
/// get additional bold + underline styling.
///
/// # Arguments
///
/// * `brackets` - Iterator of bracket info from `SharedPairState`
/// * `matched_pair` - Optional matched pair for cursor highlighting
///
/// # Returns
///
/// Vector of decorations for `DecorationGroup::Syntax`
#[must_use]
pub fn generate_bracket_decorations<'a>(
    brackets: impl Iterator<Item = &'a BracketInfo>,
    matched_pair: Option<&MatchedPair>,
) -> Vec<Decoration> {
    let mut decorations = Vec::new();

    // Track matched pair positions for special styling
    let matched_positions: Option<((usize, usize), (usize, usize))> =
        matched_pair.map(|mp| ((mp.open.line, mp.open.col), (mp.close.line, mp.close.col)));

    for bracket in brackets {
        let pos = (bracket.line, bracket.col);
        let is_matched = matched_positions.is_some_and(|(open, close)| pos == open || pos == close);

        let style = if is_matched {
            style_for_matched_pair(bracket.depth)
        } else {
            style_for_bracket(bracket.depth)
        };

        // Bracket is a single character at (line, col)
        #[allow(clippy::cast_possible_truncation)]
        let span = Span::line(bracket.line as u32, bracket.col as u32, bracket.col as u32 + 1);

        decorations.push(Decoration::inline_style(span, style));
    }

    decorations
}

/// Generate decorations from shared pair state for a specific buffer.
///
/// Convenience function that extracts bracket data from `SharedPairState`
/// and generates decorations.
///
/// # Arguments
///
/// * `state` - The shared pair state service
/// * `buffer_id` - Buffer to generate decorations for
///
/// # Returns
///
/// Vector of decorations for the buffer's brackets
#[must_use]
pub fn generate_decorations_for_buffer(
    state: &SharedPairState,
    buffer_id: reovim_kernel::api::v1::BufferId,
) -> Vec<Decoration> {
    // Get brackets and matched pair from state
    let brackets = state.get_brackets(buffer_id);
    let matched_pair = state.get_matched_pair(buffer_id);

    brackets.map_or_else(Vec::new, |bracket_map| {
        let bracket_iter = bracket_map.values();
        generate_bracket_decorations(bracket_iter, matched_pair.as_ref())
    })
}

/// The decoration group for bracket highlighting.
///
/// Uses `Syntax` group (priority 10) which is above Language (0)
/// but below Search (20) and Visual (40).
pub const PAIR_DECORATION_GROUP: DecorationGroup = DecorationGroup::Syntax;

#[cfg(test)]
mod tests {
    use reovim_driver_display::Attributes;

    use super::*;

    #[test]
    fn test_rainbow_colors_count() {
        assert_eq!(RAINBOW_COLORS.len(), 6);
    }

    #[test]
    fn test_color_for_depth_cycles() {
        // First cycle
        assert_eq!(color_for_depth(0), RAINBOW_COLORS[0]); // Red
        assert_eq!(color_for_depth(1), RAINBOW_COLORS[1]); // Orange
        assert_eq!(color_for_depth(5), RAINBOW_COLORS[5]); // Purple

        // Second cycle
        assert_eq!(color_for_depth(6), RAINBOW_COLORS[0]); // Red again
        assert_eq!(color_for_depth(7), RAINBOW_COLORS[1]); // Orange again
    }

    #[test]
    fn test_style_for_bracket_normal() {
        let style = style_for_bracket(0);
        assert_eq!(style.fg, Some(RAINBOW_COLORS[0]));
        assert!(!style.attributes.contains(Attributes::UNDERLINE));
    }

    #[test]
    fn test_style_for_bracket_unmatched() {
        let style = style_for_bracket(usize::MAX);
        assert_eq!(style.fg, Some(UNMATCHED_COLOR));
        assert!(style.attributes.contains(Attributes::UNDERLINE));
    }

    #[test]
    fn test_style_for_matched_pair() {
        let style = style_for_matched_pair(2);
        assert_eq!(style.fg, Some(RAINBOW_COLORS[2])); // Yellow
        assert!(style.attributes.contains(Attributes::BOLD));
        assert!(style.attributes.contains(Attributes::UNDERLINE));
    }

    #[test]
    fn test_generate_bracket_decorations_empty() {
        let brackets: Vec<BracketInfo> = vec![];
        let decorations = generate_bracket_decorations(brackets.iter(), None);
        assert!(decorations.is_empty());
    }

    #[test]
    fn test_generate_bracket_decorations_simple() {
        let brackets = [
            BracketInfo {
                line: 0,
                col: 0,
                depth: 0,
                char: '(',
            },
            BracketInfo {
                line: 0,
                col: 5,
                depth: 0,
                char: ')',
            },
        ];

        let decorations = generate_bracket_decorations(brackets.iter(), None);
        assert_eq!(decorations.len(), 2);

        // Both should be inline styles
        assert!(decorations[0].is_inline_style());
        assert!(decorations[1].is_inline_style());
    }

    #[test]
    fn test_generate_bracket_decorations_with_matched_pair() {
        let brackets = [
            BracketInfo {
                line: 0,
                col: 0,
                depth: 0,
                char: '(',
            },
            BracketInfo {
                line: 0,
                col: 5,
                depth: 0,
                char: ')',
            },
        ];

        let matched = MatchedPair {
            open: brackets[0],
            close: brackets[1],
        };

        let decorations = generate_bracket_decorations(brackets.iter(), Some(&matched));
        assert_eq!(decorations.len(), 2);

        // Both should have matched pair styling (bold + underline)
        // We can't easily inspect the style, but we know they're generated
        assert!(decorations[0].is_inline_style());
        assert!(decorations[1].is_inline_style());
    }

    #[test]
    fn test_pair_decoration_group() {
        assert_eq!(PAIR_DECORATION_GROUP, DecorationGroup::Syntax);
        assert_eq!(PAIR_DECORATION_GROUP.priority(), 10);
    }
}
