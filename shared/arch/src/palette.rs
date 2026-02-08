//! Colorblind-friendly palette for multi-client presence visualization.
//!
//! Uses CBF-8 palette designed to be distinguishable for all types of
//! color vision deficiency (deuteranopia, protanopia, tritanopia).
//!
//! Based on Wong (2011) "Points of view: Color blindness", Nature Methods.
//!
//! # Usage
//!
//! ```
//! use reovim_arch::palette::color_for_client;
//!
//! // Each client gets a deterministic color based on their ID
//! let color = color_for_client(42);
//! ```

use crate::Color;

/// CBF-8: Colorblind-friendly palette with 8 distinct colors.
///
/// Each color is distinguishable for all types of color vision deficiency.
/// Colors are ordered by perceptual distinctiveness.
pub const REMOTE_CLIENT_COLORS: [Color; 8] = [
    Color::Rgb {
        r: 230,
        g: 159,
        b: 0,
    }, // Orange
    Color::Rgb {
        r: 86,
        g: 180,
        b: 233,
    }, // Sky Blue
    Color::Rgb {
        r: 0,
        g: 158,
        b: 115,
    }, // Bluish Green
    Color::Rgb {
        r: 240,
        g: 228,
        b: 66,
    }, // Yellow
    Color::Rgb {
        r: 0,
        g: 114,
        b: 178,
    }, // Blue
    Color::Rgb {
        r: 213,
        g: 94,
        b: 0,
    }, // Vermillion
    Color::Rgb {
        r: 204,
        g: 121,
        b: 167,
    }, // Reddish Purple
    Color::Rgb {
        r: 0,
        g: 200,
        b: 150,
    }, // Teal
];

/// Get the assigned color for a client ID.
///
/// Colors are deterministically assigned based on `client_id % 8`.
/// This ensures the same client always gets the same color across sessions.
///
/// # Examples
///
/// ```
/// use reovim_arch::palette::color_for_client;
/// use reovim_arch::Color;
///
/// // Client 0 gets orange
/// assert!(matches!(color_for_client(0), Color::Rgb { r: 230, g: 159, b: 0 }));
///
/// // Client 8 wraps around to orange again
/// assert_eq!(color_for_client(0), color_for_client(8));
/// ```
#[must_use]
pub const fn color_for_client(client_id: u64) -> Color {
    REMOTE_CLIENT_COLORS[(client_id % 8) as usize]
}

/// Get a dimmed version of a client's color (for selection backgrounds).
///
/// Returns RGB with each component divided by 4 for a subtle overlay effect.
/// This creates a translucent appearance when used as a background color.
///
/// # Examples
///
/// ```
/// use reovim_arch::palette::dimmed_color_for_client;
/// use reovim_arch::Color;
///
/// // Dimmed orange (230/4, 159/4, 0/4)
/// assert!(matches!(dimmed_color_for_client(0), Color::Rgb { r: 57, g: 39, b: 0 }));
/// ```
#[must_use]
pub const fn dimmed_color_for_client(client_id: u64) -> Color {
    match color_for_client(client_id) {
        Color::Rgb { r, g, b } => Color::Rgb {
            r: r / 4,
            g: g / 4,
            b: b / 4,
        },
        c => c,
    }
}

/// Get a darker version of a client's color (for cursor backgrounds).
///
/// Returns RGB with each component divided by 3 for a visible but not
/// overwhelming background behind cursor characters.
#[must_use]
pub const fn dark_color_for_client(client_id: u64) -> Color {
    match color_for_client(client_id) {
        Color::Rgb { r, g, b } => Color::Rgb {
            r: r / 3,
            g: g / 3,
            b: b / 3,
        },
        c => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_assignment_deterministic() {
        // Same client ID should always get the same color
        assert_eq!(color_for_client(0), color_for_client(0));
        assert_eq!(color_for_client(42), color_for_client(42));
    }

    #[test]
    fn test_color_wraps_at_8() {
        // Colors wrap around after 8 clients
        assert_eq!(color_for_client(0), color_for_client(8));
        assert_eq!(color_for_client(1), color_for_client(9));
        assert_eq!(color_for_client(7), color_for_client(15));
    }

    #[test]
    fn test_all_colors_distinct() {
        // All 8 base colors should be different
        for i in 0..8 {
            for j in (i + 1)..8 {
                assert_ne!(
                    color_for_client(i),
                    color_for_client(j),
                    "Colors at index {i} and {j} should be different"
                );
            }
        }
    }

    #[test]
    fn test_dimmed_color() {
        // Dimmed should be 1/4 intensity
        match (color_for_client(0), dimmed_color_for_client(0)) {
            (
                Color::Rgb { r, g, b },
                Color::Rgb {
                    r: dr,
                    g: dg,
                    b: db,
                },
            ) => {
                assert_eq!(dr, r / 4);
                assert_eq!(dg, g / 4);
                assert_eq!(db, b / 4);
            }
            _ => panic!("Expected RGB colors"),
        }
    }

    #[test]
    fn test_dark_color() {
        // Dark should be 1/3 intensity
        match (color_for_client(0), dark_color_for_client(0)) {
            (
                Color::Rgb { r, g, b },
                Color::Rgb {
                    r: dr,
                    g: dg,
                    b: db,
                },
            ) => {
                assert_eq!(dr, r / 3);
                assert_eq!(dg, g / 3);
                assert_eq!(db, b / 3);
            }
            _ => panic!("Expected RGB colors"),
        }
    }

    #[test]
    fn test_palette_has_8_colors() {
        assert_eq!(REMOTE_CLIENT_COLORS.len(), 8);
    }

    #[test]
    fn test_all_palette_colors_are_rgb() {
        for (i, color) in REMOTE_CLIENT_COLORS.iter().enumerate() {
            assert!(
                matches!(color, Color::Rgb { .. }),
                "Palette color at index {i} should be RGB, got: {color:?}"
            );
        }
    }

    #[test]
    fn test_color_for_client_large_ids() {
        // Very large client IDs should wrap correctly
        assert_eq!(color_for_client(u64::MAX), color_for_client(u64::MAX % 8));
        assert_eq!(color_for_client(1_000_000), color_for_client(1_000_000 % 8));
    }

    #[test]
    fn test_dimmed_color_all_clients() {
        for id in 0..8 {
            match (color_for_client(id), dimmed_color_for_client(id)) {
                (
                    Color::Rgb { r, g, b },
                    Color::Rgb {
                        r: dr,
                        g: dg,
                        b: db,
                    },
                ) => {
                    assert_eq!(dr, r / 4, "Dimmed red mismatch for client {id}");
                    assert_eq!(dg, g / 4, "Dimmed green mismatch for client {id}");
                    assert_eq!(db, b / 4, "Dimmed blue mismatch for client {id}");
                }
                _ => panic!("Expected RGB colors for client {id}"),
            }
        }
    }

    #[test]
    fn test_dark_color_all_clients() {
        for id in 0..8 {
            match (color_for_client(id), dark_color_for_client(id)) {
                (
                    Color::Rgb { r, g, b },
                    Color::Rgb {
                        r: dr,
                        g: dg,
                        b: db,
                    },
                ) => {
                    assert_eq!(dr, r / 3, "Dark red mismatch for client {id}");
                    assert_eq!(dg, g / 3, "Dark green mismatch for client {id}");
                    assert_eq!(db, b / 3, "Dark blue mismatch for client {id}");
                }
                _ => panic!("Expected RGB colors for client {id}"),
            }
        }
    }

    #[test]
    fn test_dimmed_wraps_same_as_color() {
        assert_eq!(dimmed_color_for_client(0), dimmed_color_for_client(8));
        assert_eq!(dimmed_color_for_client(3), dimmed_color_for_client(11));
    }

    #[test]
    fn test_dark_wraps_same_as_color() {
        assert_eq!(dark_color_for_client(0), dark_color_for_client(8));
        assert_eq!(dark_color_for_client(5), dark_color_for_client(13));
    }

    #[test]
    fn test_dimmed_is_darker_than_original() {
        for id in 0..8 {
            match (color_for_client(id), dimmed_color_for_client(id)) {
                (
                    Color::Rgb { r, g, b },
                    Color::Rgb {
                        r: dr,
                        g: dg,
                        b: db,
                    },
                ) => {
                    assert!(dr <= r, "Dimmed red should be <= original for client {id}");
                    assert!(dg <= g, "Dimmed green should be <= original for client {id}");
                    assert!(db <= b, "Dimmed blue should be <= original for client {id}");
                }
                _ => panic!("Expected RGB colors for client {id}"),
            }
        }
    }

    #[test]
    fn test_first_color_is_orange() {
        // First palette color should be orange (230, 159, 0) per CBF-8
        assert_eq!(
            color_for_client(0),
            Color::Rgb {
                r: 230,
                g: 159,
                b: 0
            }
        );
    }
}
