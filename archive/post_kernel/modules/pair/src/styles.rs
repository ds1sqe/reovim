//! Style group definitions for the pair module.
//!
//! This module defines the style group constants and default styles for
//! rainbow bracket highlighting. These are registered with `StyleGroupRegistry`
//! during module init, allowing themes to optionally override them.
//!
//! # Architecture
//!
//! This follows mechanism vs policy separation:
//!
//! - **Mechanism** (`StyleGroupRegistry` in driver): Generic registry for style groups
//! - **Policy** (this module): Specific group names and default colors for rainbow brackets
//!
//! # Groups
//!
//! - `rainbow.bracket.{1-6}`: Bracket colors by depth (cycles for deeper nesting)
//! - `bracket.unmatched`: Warning style for unmatched brackets
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_pair::styles;
//!
//! // Get group name for depth 2 bracket
//! let group = styles::groups::RAINBOW_BRACKET_3;  // "rainbow.bracket.3"
//!
//! // Register defaults with StyleGroupRegistry
//! for (group, style) in styles::default_registrations() {
//!     registry.register(group, style);
//! }
//! ```

use {reovim_arch::Color, reovim_driver_display::Style};

/// Style group name constants for rainbow brackets.
///
/// These group names follow the `<feature>.<subtype>.<variant>` convention.
pub mod groups {
    /// Rainbow bracket depth 1 (red)
    pub const RAINBOW_BRACKET_1: &str = "rainbow.bracket.1";

    /// Rainbow bracket depth 2 (orange)
    pub const RAINBOW_BRACKET_2: &str = "rainbow.bracket.2";

    /// Rainbow bracket depth 3 (yellow)
    pub const RAINBOW_BRACKET_3: &str = "rainbow.bracket.3";

    /// Rainbow bracket depth 4 (green)
    pub const RAINBOW_BRACKET_4: &str = "rainbow.bracket.4";

    /// Rainbow bracket depth 5 (blue)
    pub const RAINBOW_BRACKET_5: &str = "rainbow.bracket.5";

    /// Rainbow bracket depth 6 (purple)
    pub const RAINBOW_BRACKET_6: &str = "rainbow.bracket.6";

    /// Unmatched bracket (red + underline warning)
    pub const BRACKET_UNMATCHED: &str = "bracket.unmatched";

    /// All rainbow bracket groups (for iteration).
    pub const ALL_RAINBOW_GROUPS: &[&str] = &[
        RAINBOW_BRACKET_1,
        RAINBOW_BRACKET_2,
        RAINBOW_BRACKET_3,
        RAINBOW_BRACKET_4,
        RAINBOW_BRACKET_5,
        RAINBOW_BRACKET_6,
        BRACKET_UNMATCHED,
    ];
}

/// Get the style group name for a bracket depth.
///
/// Returns the appropriate `rainbow.bracket.N` group name, cycling for depths >= 6.
#[must_use]
pub const fn group_for_depth(depth: usize) -> &'static str {
    match depth % 6 {
        0 => groups::RAINBOW_BRACKET_1,
        1 => groups::RAINBOW_BRACKET_2,
        2 => groups::RAINBOW_BRACKET_3,
        3 => groups::RAINBOW_BRACKET_4,
        4 => groups::RAINBOW_BRACKET_5,
        5 => groups::RAINBOW_BRACKET_6,
        _ => unreachable!(),
    }
}

/// Generate the default style registrations for rainbow brackets.
///
/// These are the default colors used when the theme doesn't define
/// custom colors for rainbow brackets. The pair module registers
/// these in `StyleGroupRegistry` during init.
///
/// # Colors
///
/// Uses a vibrant 6-color palette designed for visibility:
/// - Depth 1: Red (#e53935)
/// - Depth 2: Orange (#ff9800)
/// - Depth 3: Yellow (#fdd835)
/// - Depth 4: Green (#4caf50)
/// - Depth 5: Blue (#2196f3)
/// - Depth 6: Purple (#9c27b0)
/// - Unmatched: Red with underline
#[must_use]
pub fn default_registrations() -> Vec<(&'static str, Style)> {
    vec![
        (
            groups::RAINBOW_BRACKET_1,
            Style::new().fg(Color::Rgb {
                r: 229,
                g: 57,
                b: 53,
            }), // Red
        ),
        (
            groups::RAINBOW_BRACKET_2,
            Style::new().fg(Color::Rgb {
                r: 255,
                g: 152,
                b: 0,
            }), // Orange
        ),
        (
            groups::RAINBOW_BRACKET_3,
            Style::new().fg(Color::Rgb {
                r: 253,
                g: 216,
                b: 53,
            }), // Yellow
        ),
        (
            groups::RAINBOW_BRACKET_4,
            Style::new().fg(Color::Rgb {
                r: 76,
                g: 175,
                b: 80,
            }), // Green
        ),
        (
            groups::RAINBOW_BRACKET_5,
            Style::new().fg(Color::Rgb {
                r: 33,
                g: 150,
                b: 243,
            }), // Blue
        ),
        (
            groups::RAINBOW_BRACKET_6,
            Style::new().fg(Color::Rgb {
                r: 156,
                g: 39,
                b: 176,
            }), // Purple
        ),
        (
            groups::BRACKET_UNMATCHED,
            Style::new()
                .fg(Color::Rgb {
                    r: 224,
                    g: 108,
                    b: 117,
                }) // Red (like error)
                .underline(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_for_depth_cycles() {
        assert_eq!(group_for_depth(0), groups::RAINBOW_BRACKET_1);
        assert_eq!(group_for_depth(1), groups::RAINBOW_BRACKET_2);
        assert_eq!(group_for_depth(5), groups::RAINBOW_BRACKET_6);
        // Cycles
        assert_eq!(group_for_depth(6), groups::RAINBOW_BRACKET_1);
        assert_eq!(group_for_depth(7), groups::RAINBOW_BRACKET_2);
    }

    #[test]
    fn test_default_registrations_count() {
        let registrations = default_registrations();
        assert_eq!(registrations.len(), 7); // 6 rainbow + 1 unmatched
    }

    #[test]
    fn test_all_rainbow_groups_covered() {
        let registrations = default_registrations();
        for group in groups::ALL_RAINBOW_GROUPS {
            assert!(
                registrations.iter().any(|(g, _)| g == group),
                "Missing registration for group: {group}"
            );
        }
    }
}
