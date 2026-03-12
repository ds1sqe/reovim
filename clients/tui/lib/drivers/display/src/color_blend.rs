//! Color blending utilities for layer transparency.
//!
//! Provides functions to blend colors toward a background color based on
//! opacity. Used by the TUI display driver to render semi-transparent layers
//! by dimming cell colors toward the terminal default background.
//!
//! # Algorithm
//!
//! ```text
//! effective_color = lerp(default_bg, cell_color, opacity)
//!
//! At opacity=1.0: color unchanged (fully opaque)
//! At opacity=0.5: color halfway to background (semi-transparent)
//! At opacity=0.0: color equals background (fully transparent)
//! ```

use crate::{Color, Style};

/// ANSI named color to approximate RGB mapping.
///
/// Used when blending ANSI named colors that don't have native RGB values.
/// These are standard terminal color approximations.
const fn ansi_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::DarkRed => (128, 0, 0),
        Color::DarkGreen => (0, 128, 0),
        Color::DarkYellow => (128, 128, 0),
        Color::DarkBlue => (0, 0, 128),
        Color::DarkMagenta => (128, 0, 128),
        Color::DarkCyan => (0, 128, 128),
        Color::Grey => (192, 192, 192),
        Color::DarkGrey => (128, 128, 128),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 255, 0),
        Color::Yellow => (255, 255, 0),
        Color::Blue => (0, 0, 255),
        Color::Magenta => (255, 0, 255),
        Color::Cyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        Color::Rgb { r, g, b } => (r, g, b),
        // Black, Reset, and AnsiValue: treat as black for blending
        Color::Black | Color::Reset | Color::AnsiValue(_) => (0, 0, 0),
    }
}

/// Linear interpolation between two colors.
///
/// Blends `from` toward `to` by factor `t`:
/// - `t=0.0` → returns `from`
/// - `t=1.0` → returns `to`
/// - `t=0.5` → midpoint between `from` and `to`
///
/// ANSI named colors are converted to approximate RGB for blending.
/// `Color::Reset` is treated as black (0,0,0).
#[must_use]
pub fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);

    // Fast paths: no blending needed
    if (t - 1.0).abs() < f32::EPSILON {
        return to;
    }
    if t.abs() < f32::EPSILON {
        return from;
    }

    let (fr, fg, fb) = ansi_to_rgb(from);
    let (tr, tg, tb) = ansi_to_rgb(to);

    let lerp_u8 = |a: u8, b: u8| -> u8 {
        let a = f32::from(a);
        let b = f32::from(b);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let result = (b - a).mul_add(t, a) as u8;
        result
    };

    Color::Rgb {
        r: lerp_u8(fr, tr),
        g: lerp_u8(fg, tg),
        b: lerp_u8(fb, tb),
    }
}

/// Apply opacity dimming to a style.
///
/// Blends the style's foreground and background colors toward `default_bg`
/// proportionally to `(1 - opacity)`. Preserves text attributes (bold,
/// italic, underline, etc.).
///
/// - `opacity=1.0` → style unchanged
/// - `opacity=0.5` → colors halfway to background
/// - `opacity=0.0` → all colors become background
#[must_use]
pub fn dim_style(style: &Style, opacity: f32, default_bg: Color) -> Style {
    if (opacity - 1.0).abs() < f32::EPSILON {
        return style.clone();
    }

    let blend_opt = |color: Option<Color>| -> Option<Color> {
        color.map(|c| lerp_color(default_bg, c, opacity))
    };

    Style {
        fg: blend_opt(style.fg),
        bg: blend_opt(style.bg),
        attributes: style.attributes,
        underline_color: blend_opt(style.underline_color),
    }
}

#[cfg(test)]
#[path = "color_blend_tests.rs"]
mod tests;
