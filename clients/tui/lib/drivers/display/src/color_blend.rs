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
mod tests {
    use {super::*, crate::Attributes};

    fn bold() -> Attributes {
        let mut a = Attributes::new();
        a.set(Attributes::BOLD);
        a
    }

    fn italic() -> Attributes {
        let mut a = Attributes::new();
        a.set(Attributes::ITALIC);
        a
    }

    #[test]
    fn test_lerp_color_rgb_full_opacity() {
        let from = Color::Black;
        let to = Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        };
        let result = lerp_color(from, to, 1.0);
        assert_eq!(
            result,
            Color::Rgb {
                r: 200,
                g: 100,
                b: 50
            }
        );
    }

    #[test]
    fn test_lerp_color_rgb_zero_opacity() {
        let from = Color::Black;
        let to = Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        };
        let result = lerp_color(from, to, 0.0);
        assert_eq!(result, Color::Black);
    }

    #[test]
    fn test_lerp_color_rgb_midpoint() {
        let from = Color::Rgb { r: 0, g: 0, b: 0 };
        let to = Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        };
        let result = lerp_color(from, to, 0.5);
        assert_eq!(
            result,
            Color::Rgb {
                r: 100,
                g: 50,
                b: 25
            }
        );
    }

    #[test]
    fn test_lerp_color_ansi_named() {
        let from = Color::Black;
        let to = Color::White;
        let result = lerp_color(from, to, 0.5);
        // Black (0,0,0) -> White (255,255,255) at 0.5 = (127,127,127)
        assert_eq!(
            result,
            Color::Rgb {
                r: 127,
                g: 127,
                b: 127
            }
        );
    }

    #[test]
    fn test_lerp_color_clamps_t() {
        let from = Color::Black;
        let to = Color::White;

        // t > 1.0 should clamp to 1.0
        let result = lerp_color(from, to, 2.0);
        assert_eq!(result, Color::White);

        // t < 0.0 should clamp to 0.0
        let result = lerp_color(from, to, -1.0);
        assert_eq!(result, Color::Black);
    }

    #[test]
    fn test_lerp_color_reset() {
        let from = Color::Reset;
        let to = Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        };
        let result = lerp_color(from, to, 0.5);
        assert_eq!(
            result,
            Color::Rgb {
                r: 50,
                g: 50,
                b: 50
            }
        );
    }

    #[test]
    fn test_lerp_color_ansi_value() {
        let from = Color::AnsiValue(42);
        let to = Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        };
        // AnsiValue is approximated as black for blending
        let result = lerp_color(from, to, 0.5);
        assert_eq!(
            result,
            Color::Rgb {
                r: 50,
                g: 50,
                b: 50
            }
        );
    }

    #[test]
    fn test_dim_style_full_opacity() {
        let style = Style {
            fg: Some(Color::Red),
            bg: Some(Color::Blue),
            attributes: bold(),
            underline_color: None,
        };
        let result = dim_style(&style, 1.0, Color::Black);
        assert_eq!(result.fg, style.fg);
        assert_eq!(result.bg, style.bg);
        assert_eq!(result.attributes, bold());
    }

    #[test]
    fn test_dim_style_zero_opacity() {
        let style = Style {
            fg: Some(Color::White),
            bg: Some(Color::Blue),
            attributes: italic(),
            underline_color: Some(Color::Red),
        };
        let bg = Color::Black;
        let result = dim_style(&style, 0.0, bg);

        // All colors should be background (black)
        assert_eq!(result.fg, Some(Color::Black));
        assert_eq!(result.bg, Some(Color::Black));
        assert_eq!(result.underline_color, Some(Color::Black));
        // Attributes preserved
        assert_eq!(result.attributes, italic());
    }

    #[test]
    fn test_dim_style_half_opacity() {
        let style = Style {
            fg: Some(Color::Rgb {
                r: 200,
                g: 100,
                b: 50,
            }),
            bg: None,
            attributes: Attributes::new(),
            underline_color: None,
        };
        let bg = Color::Rgb { r: 0, g: 0, b: 0 };
        let result = dim_style(&style, 0.5, bg);

        assert_eq!(
            result.fg,
            Some(Color::Rgb {
                r: 100,
                g: 50,
                b: 25
            })
        );
        assert_eq!(result.bg, None);
    }

    #[test]
    fn test_dim_style_preserves_none_colors() {
        let style = Style {
            fg: None,
            bg: None,
            attributes: Attributes::new(),
            underline_color: None,
        };
        let result = dim_style(&style, 0.5, Color::Black);

        assert_eq!(result.fg, None);
        assert_eq!(result.bg, None);
        assert_eq!(result.underline_color, None);
    }

    #[test]
    fn test_lerp_color_dark_ansi_colors() {
        // At t=0.5, ANSI colors get converted to RGB via approximation
        let colors = [
            Color::DarkRed,
            Color::DarkGreen,
            Color::DarkYellow,
            Color::DarkBlue,
            Color::DarkMagenta,
            Color::DarkCyan,
            Color::DarkGrey,
            Color::Grey,
        ];

        for color in &colors {
            let result = lerp_color(Color::Black, *color, 0.5);
            assert!(matches!(result, Color::Rgb { .. }), "color {color:?}");
        }
    }

    #[test]
    fn test_lerp_color_bright_ansi_colors() {
        let colors = [
            Color::Red,
            Color::Green,
            Color::Yellow,
            Color::Blue,
            Color::Magenta,
            Color::Cyan,
        ];

        for color in &colors {
            let result = lerp_color(Color::Black, *color, 0.5);
            assert!(matches!(result, Color::Rgb { .. }), "color {color:?}");
        }
    }
}
