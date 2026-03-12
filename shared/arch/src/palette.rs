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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[path = "palette_tests.rs"]
mod tests;
