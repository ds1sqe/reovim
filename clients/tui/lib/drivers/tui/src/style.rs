//! Style types for TUI rendering.
//!
//! Simplified style system for terminal display. Uses the Color type from
//! reovim-arch for platform abstraction.

use reovim_arch::Color;

/// Terminal color capability levels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ColorMode {
    /// 16 ANSI colors (basic terminal).
    Ansi16,
    /// 256 color palette.
    Color256,
    /// 24-bit true color (RGB).
    #[default]
    TrueColor,
}

impl ColorMode {
    /// Detect terminal color capability from environment variables.
    #[must_use]
    pub fn detect() -> Self {
        // Check COLORTERM first (most specific)
        if let Ok(colorterm) = std::env::var("COLORTERM") {
            let ct = colorterm.to_lowercase();
            if ct == "truecolor" || ct == "24bit" {
                return Self::TrueColor;
            }
        }

        // Check TERM for 256color
        if let Ok(term) = std::env::var("TERM")
            && term.contains("256color")
        {
            return Self::Color256;
        }

        // Fall back to basic ANSI
        Self::Ansi16
    }
}

/// Bitflags for text attributes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attributes(u16);

impl Attributes {
    pub const BOLD: u16 = 1 << 0;
    pub const ITALIC: u16 = 1 << 1;
    pub const UNDERLINE: u16 = 1 << 2;
    pub const STRIKETHROUGH: u16 = 1 << 3;
    pub const REVERSE: u16 = 1 << 4;
    pub const BLINK: u16 = 1 << 5;
    pub const DIM: u16 = 1 << 6;

    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn contains(self, attr: u16) -> bool {
        (self.0 & attr) != 0
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn set(&mut self, attr: u16) {
        self.0 |= attr;
    }
}

/// Text style with foreground, background, and attributes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    /// Foreground color.
    pub fg: Option<Color>,
    /// Background color.
    pub bg: Option<Color>,
    /// Text attributes.
    pub attrs: Attributes,
}

impl Style {
    /// Create a new default style.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            attrs: Attributes::new(),
        }
    }

    /// Set foreground color.
    #[must_use]
    pub const fn with_fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    /// Set background color.
    #[must_use]
    pub const fn with_bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Add bold attribute.
    #[must_use]
    pub fn bold(mut self) -> Self {
        self.attrs.set(Attributes::BOLD);
        self
    }

    /// Add italic attribute.
    #[must_use]
    pub fn italic(mut self) -> Self {
        self.attrs.set(Attributes::ITALIC);
        self
    }

    /// Add underline attribute.
    #[must_use]
    pub fn underline(mut self) -> Self {
        self.attrs.set(Attributes::UNDERLINE);
        self
    }

    /// Add reverse attribute.
    #[must_use]
    pub fn reverse(mut self) -> Self {
        self.attrs.set(Attributes::REVERSE);
        self
    }

    /// Add dim attribute.
    #[must_use]
    pub fn dim(mut self) -> Self {
        self.attrs.set(Attributes::DIM);
        self
    }

    /// Convert to ANSI escape sequence start.
    #[must_use]
    pub fn to_ansi_start(&self, mode: ColorMode) -> String {
        let mut codes = Vec::new();

        // Attributes
        if self.attrs.contains(Attributes::BOLD) {
            codes.push("1".to_string());
        }
        if self.attrs.contains(Attributes::DIM) {
            codes.push("2".to_string());
        }
        if self.attrs.contains(Attributes::ITALIC) {
            codes.push("3".to_string());
        }
        if self.attrs.contains(Attributes::UNDERLINE) {
            codes.push("4".to_string());
        }
        if self.attrs.contains(Attributes::BLINK) {
            codes.push("5".to_string());
        }
        if self.attrs.contains(Attributes::REVERSE) {
            codes.push("7".to_string());
        }
        if self.attrs.contains(Attributes::STRIKETHROUGH) {
            codes.push("9".to_string());
        }

        // Foreground color
        if let Some(color) = self.fg {
            codes.push(Self::color_to_ansi(color, true, mode));
        }

        // Background color
        if let Some(color) = self.bg {
            codes.push(Self::color_to_ansi(color, false, mode));
        }

        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }

    /// Convert color to ANSI code.
    #[allow(clippy::match_same_arms)]
    fn color_to_ansi(color: Color, fg: bool, mode: ColorMode) -> String {
        // Convert named colors to their ANSI codes
        let ansi_code = match color {
            Color::Reset => {
                return if fg {
                    "39".to_string()
                } else {
                    "49".to_string()
                };
            }
            Color::Black => 0,
            Color::DarkRed => 1,
            Color::DarkGreen => 2,
            Color::DarkYellow => 3,
            Color::DarkBlue => 4,
            Color::DarkMagenta => 5,
            Color::DarkCyan => 6,
            Color::Grey => 7,
            Color::DarkGrey => 8,
            Color::Red => 9,
            Color::Green => 10,
            Color::Yellow => 11,
            Color::Blue => 12,
            Color::Magenta => 13,
            Color::Cyan => 14,
            Color::White => 15,
            Color::AnsiValue(idx) => {
                return if fg {
                    format!("38;5;{idx}")
                } else {
                    format!("48;5;{idx}")
                };
            }
            Color::Rgb { r, g, b } => {
                return match mode {
                    ColorMode::TrueColor => {
                        if fg {
                            format!("38;2;{r};{g};{b}")
                        } else {
                            format!("48;2;{r};{g};{b}")
                        }
                    }
                    ColorMode::Color256 => {
                        let idx = rgb_to_256(r, g, b);
                        if fg {
                            format!("38;5;{idx}")
                        } else {
                            format!("48;5;{idx}")
                        }
                    }
                    ColorMode::Ansi16 => {
                        let base = rgb_to_ansi16(r, g, b);
                        if fg {
                            format!("{base}")
                        } else {
                            format!("{}", base + 10)
                        }
                    }
                };
            }
        };

        // Standard 16-color handling
        if ansi_code < 8 {
            // Colors 0-7: 30-37 (fg) or 40-47 (bg)
            let base = if fg { 30 } else { 40 };
            format!("{}", base + ansi_code)
        } else {
            // Colors 8-15: 90-97 (fg) or 100-107 (bg)
            let base = if fg { 90 } else { 100 };
            format!("{}", base + ansi_code - 8)
        }
    }
}

/// Convert RGB to 256-color palette index.
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    // Use the 6x6x6 color cube (indices 16-231)
    let r_idx = (u16::from(r) * 6 / 256) as u8;
    let g_idx = (u16::from(g) * 6 / 256) as u8;
    let b_idx = (u16::from(b) * 6 / 256) as u8;
    16 + 36 * r_idx + 6 * g_idx + b_idx
}

/// Convert RGB to ANSI 16-color code.
fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> u8 {
    let brightness = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
    let bright = brightness > 127;

    // Determine base color
    let r_on = r > 127;
    let g_on = g > 127;
    let b_on = b > 127;

    let base = match (r_on, g_on, b_on) {
        (false, false, false) => 30, // black
        (true, false, false) => 31,  // red
        (false, true, false) => 32,  // green
        (true, true, false) => 33,   // yellow
        (false, false, true) => 34,  // blue
        (true, false, true) => 35,   // magenta
        (false, true, true) => 36,   // cyan
        (true, true, true) => 37,    // white
    };

    if bright && base < 37 {
        base + 60 // Bright variant
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_default() {
        let style = Style::default();
        assert!(style.fg.is_none());
        assert!(style.bg.is_none());
    }

    #[test]
    fn test_style_with_attrs() {
        let style = Style::new().bold().italic();
        assert!(style.attrs.contains(Attributes::BOLD));
        assert!(style.attrs.contains(Attributes::ITALIC));
    }

    #[test]
    fn test_style_to_ansi() {
        let style = Style::new().bold();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('1')); // Bold code
    }

    #[test]
    fn test_color_mode_detect() {
        // Just ensure it doesn't panic
        let _ = ColorMode::detect();
    }

    #[test]
    fn test_color_mode_default() {
        assert_eq!(ColorMode::default(), ColorMode::TrueColor);
    }

    #[test]
    fn test_color_mode_eq() {
        assert_eq!(ColorMode::Ansi16, ColorMode::Ansi16);
        assert_eq!(ColorMode::Color256, ColorMode::Color256);
        assert_eq!(ColorMode::TrueColor, ColorMode::TrueColor);
        assert_ne!(ColorMode::Ansi16, ColorMode::Color256);
    }

    #[test]
    fn test_attributes_new() {
        let attrs = Attributes::new();
        assert!(!attrs.contains(Attributes::BOLD));
        assert!(!attrs.contains(Attributes::ITALIC));
    }

    #[test]
    fn test_attributes_set() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::BOLD);
        assert!(attrs.contains(Attributes::BOLD));
        assert!(!attrs.contains(Attributes::ITALIC));

        attrs.set(Attributes::ITALIC);
        assert!(attrs.contains(Attributes::BOLD));
        assert!(attrs.contains(Attributes::ITALIC));
    }

    #[test]
    fn test_attributes_all_flags() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::BOLD);
        attrs.set(Attributes::ITALIC);
        attrs.set(Attributes::UNDERLINE);
        attrs.set(Attributes::STRIKETHROUGH);
        attrs.set(Attributes::REVERSE);
        attrs.set(Attributes::BLINK);
        attrs.set(Attributes::DIM);

        assert!(attrs.contains(Attributes::BOLD));
        assert!(attrs.contains(Attributes::ITALIC));
        assert!(attrs.contains(Attributes::UNDERLINE));
        assert!(attrs.contains(Attributes::STRIKETHROUGH));
        assert!(attrs.contains(Attributes::REVERSE));
        assert!(attrs.contains(Attributes::BLINK));
        assert!(attrs.contains(Attributes::DIM));
    }

    #[test]
    fn test_style_new() {
        let style = Style::new();
        assert!(style.fg.is_none());
        assert!(style.bg.is_none());
        assert_eq!(style.attrs, Attributes::new());
    }

    #[test]
    fn test_style_with_fg() {
        let style = Style::new().with_fg(Color::Red);
        assert_eq!(style.fg, Some(Color::Red));
        assert!(style.bg.is_none());
    }

    #[test]
    fn test_style_with_bg() {
        let style = Style::new().with_bg(Color::Blue);
        assert_eq!(style.bg, Some(Color::Blue));
        assert!(style.fg.is_none());
    }

    #[test]
    fn test_style_chaining() {
        let style = Style::new()
            .with_fg(Color::Red)
            .with_bg(Color::Blue)
            .bold()
            .italic()
            .underline();

        assert_eq!(style.fg, Some(Color::Red));
        assert_eq!(style.bg, Some(Color::Blue));
        assert!(style.attrs.contains(Attributes::BOLD));
        assert!(style.attrs.contains(Attributes::ITALIC));
        assert!(style.attrs.contains(Attributes::UNDERLINE));
    }

    #[test]
    fn test_style_reverse() {
        let style = Style::new().reverse();
        assert!(style.attrs.contains(Attributes::REVERSE));
    }

    #[test]
    fn test_style_dim() {
        let style = Style::new().dim();
        assert!(style.attrs.contains(Attributes::DIM));
    }

    #[test]
    fn test_style_to_ansi_empty() {
        let style = Style::new();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert_eq!(ansi, "");
    }

    #[test]
    fn test_style_to_ansi_bold_only() {
        let style = Style::new().bold();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('1'));
        assert!(ansi.starts_with("\x1b["));
        assert!(ansi.ends_with('m'));
    }

    #[test]
    fn test_style_to_ansi_all_attrs() {
        let style = Style::new().bold().dim().italic().underline().reverse();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('1')); // bold
        assert!(ansi.contains('2')); // dim
        assert!(ansi.contains('3')); // italic
        assert!(ansi.contains('4')); // underline
        assert!(ansi.contains('7')); // reverse
    }

    #[test]
    fn test_style_to_ansi_with_fg_truecolor() {
        let style = Style::new().with_fg(Color::Rgb {
            r: 255,
            g: 128,
            b: 64,
        });
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("38;2;255;128;64"));
    }

    #[test]
    fn test_style_to_ansi_with_bg_truecolor() {
        let style = Style::new().with_bg(Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        });
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("48;2;10;20;30"));
    }

    #[test]
    fn test_style_to_ansi_rgb_color256() {
        let style = Style::new().with_fg(Color::Rgb { r: 255, g: 0, b: 0 });
        let ansi = style.to_ansi_start(ColorMode::Color256);
        assert!(ansi.contains("38;5;"));
    }

    #[test]
    fn test_style_to_ansi_rgb_ansi16() {
        let style = Style::new().with_fg(Color::Rgb { r: 255, g: 0, b: 0 });
        let ansi = style.to_ansi_start(ColorMode::Ansi16);
        // Should map to red (31 or 91)
        assert!(ansi.contains("31") || ansi.contains("91"));
    }

    #[test]
    fn test_style_to_ansi_named_colors() {
        let style = Style::new().with_fg(Color::Red);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("91")); // Bright red

        let style = Style::new().with_fg(Color::Black);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("30"));

        let style = Style::new().with_fg(Color::White);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("97"));
    }

    #[test]
    fn test_style_to_ansi_reset_color() {
        let style = Style::new().with_fg(Color::Reset);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("39"));

        let style = Style::new().with_bg(Color::Reset);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("49"));
    }

    #[test]
    fn test_style_to_ansi_ansi_value() {
        let style = Style::new().with_fg(Color::AnsiValue(42));
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("38;5;42"));

        let style = Style::new().with_bg(Color::AnsiValue(100));
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("48;5;100"));
    }

    #[test]
    fn test_rgb_to_256() {
        // Black should map to 16
        let idx = rgb_to_256(0, 0, 0);
        assert_eq!(idx, 16);

        // White should map to 231
        let idx = rgb_to_256(255, 255, 255);
        assert_eq!(idx, 231);

        // Pure red
        let idx = rgb_to_256(255, 0, 0);
        assert!((16..=231).contains(&idx));
    }

    #[test]
    fn test_rgb_to_ansi16_black() {
        let code = rgb_to_ansi16(0, 0, 0);
        assert_eq!(code, 30); // Black
    }

    #[test]
    fn test_rgb_to_ansi16_white() {
        let code = rgb_to_ansi16(255, 255, 255);
        // brightness = (255+255+255)/3 = 255 > 127, bright=true
        // all on => base=37, bright => 37 (base < 37 is false)
        assert_eq!(code, 37); // White (not bright because base == 37)
    }

    #[test]
    fn test_rgb_to_ansi16_red() {
        let code = rgb_to_ansi16(255, 0, 0);
        // brightness = 255/3 = 85 <= 127, bright=false
        // r_on=true, g_on=false, b_on=false => base=31
        assert_eq!(code, 31); // Red
    }

    #[test]
    fn test_rgb_to_ansi16_green() {
        let code = rgb_to_ansi16(0, 255, 0);
        // brightness = 255/3 = 85 <= 127, bright=false
        // r_on=false, g_on=true, b_on=false => base=32
        assert_eq!(code, 32); // Green
    }

    #[test]
    fn test_rgb_to_ansi16_blue() {
        let code = rgb_to_ansi16(0, 0, 255);
        // brightness = 255/3 = 85 <= 127, bright=false
        // r_on=false, g_on=false, b_on=true => base=34
        assert_eq!(code, 34); // Blue
    }

    #[test]
    fn test_rgb_to_ansi16_yellow() {
        let code = rgb_to_ansi16(255, 255, 0);
        assert_eq!(code, 93); // Bright yellow
    }

    #[test]
    fn test_rgb_to_ansi16_cyan() {
        let code = rgb_to_ansi16(0, 255, 255);
        assert_eq!(code, 96); // Bright cyan
    }

    #[test]
    fn test_rgb_to_ansi16_magenta() {
        let code = rgb_to_ansi16(255, 0, 255);
        assert_eq!(code, 95); // Bright magenta
    }

    #[test]
    fn test_rgb_to_ansi16_dark_red() {
        let code = rgb_to_ansi16(127, 0, 0);
        // brightness = 127/3 = 42 <= 127, bright=false
        // r_on=false (127 is not > 127), g_on=false, b_on=false => base=30 (black)
        assert_eq!(code, 30); // Black (because 127 is not > 127)
    }

    #[test]
    fn test_style_clone() {
        let style = Style::new().with_fg(Color::Red).bold();
        let cloned = style.clone();
        assert_eq!(style, cloned);
    }

    #[test]
    fn test_style_eq() {
        let style1 = Style::new().with_fg(Color::Red);
        let style2 = Style::new().with_fg(Color::Red);
        assert_eq!(style1, style2);

        let style3 = Style::new().with_fg(Color::Blue);
        assert_ne!(style1, style3);
    }

    #[test]
    fn test_attributes_default() {
        let attrs = Attributes::default();
        assert!(!attrs.contains(Attributes::BOLD));
    }

    #[test]
    fn test_attributes_clone() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::BOLD);
        let cloned = attrs;
        assert_eq!(attrs, cloned);
    }

    #[test]
    fn test_color_mode_clone() {
        let mode = ColorMode::TrueColor;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_color_mode_debug() {
        let mode = ColorMode::TrueColor;
        let debug = format!("{mode:?}");
        assert!(debug.contains("TrueColor"));
    }

    #[test]
    fn test_style_debug() {
        let style = Style::new().with_fg(Color::Red);
        let debug = format!("{style:?}");
        assert!(debug.contains("Style"));
    }

    #[test]
    fn test_attributes_debug() {
        let attrs = Attributes::new();
        let debug = format!("{attrs:?}");
        assert!(debug.contains("Attributes"));
    }

    #[test]
    fn test_style_to_ansi_blink() {
        let mut style = Style::new();
        style.attrs.set(Attributes::BLINK);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('5')); // Blink code
    }

    #[test]
    fn test_style_to_ansi_strikethrough() {
        let mut style = Style::new();
        style.attrs.set(Attributes::STRIKETHROUGH);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('9')); // Strikethrough code
    }

    #[test]
    fn test_style_to_ansi_all_attrs_including_blink_strikethrough() {
        let mut style = Style::new();
        style.attrs.set(Attributes::BOLD);
        style.attrs.set(Attributes::DIM);
        style.attrs.set(Attributes::ITALIC);
        style.attrs.set(Attributes::UNDERLINE);
        style.attrs.set(Attributes::BLINK);
        style.attrs.set(Attributes::REVERSE);
        style.attrs.set(Attributes::STRIKETHROUGH);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        // Expected: \x1b[1;2;3;4;5;7;9m
        assert_eq!(ansi, "\x1b[1;2;3;4;5;7;9m");
    }

    #[test]
    fn test_color_to_ansi_named_bg() {
        // Test background colors for named colors
        let style = Style::new().with_bg(Color::Black);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("40"));

        let style = Style::new().with_bg(Color::Red);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("101")); // Bright red bg (ANSI 9 -> 100 + 9 - 8 = 101)

        let style = Style::new().with_bg(Color::White);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("107")); // Bright white bg
    }

    #[test]
    fn test_color_to_ansi_all_dark_colors_fg() {
        let colors_and_expected: [(Color, &str); 8] = [
            (Color::Black, "30"),
            (Color::DarkRed, "31"),
            (Color::DarkGreen, "32"),
            (Color::DarkYellow, "33"),
            (Color::DarkBlue, "34"),
            (Color::DarkMagenta, "35"),
            (Color::DarkCyan, "36"),
            (Color::Grey, "37"),
        ];
        for (color, expected) in colors_and_expected {
            let style = Style::new().with_fg(color);
            let ansi = style.to_ansi_start(ColorMode::TrueColor);
            assert!(
                ansi.contains(expected),
                "Color {color:?} should contain {expected}, got {ansi}"
            );
        }
    }

    #[test]
    fn test_color_to_ansi_all_bright_colors_fg() {
        let colors_and_expected: [(Color, &str); 8] = [
            (Color::DarkGrey, "90"),
            (Color::Red, "91"),
            (Color::Green, "92"),
            (Color::Yellow, "93"),
            (Color::Blue, "94"),
            (Color::Magenta, "95"),
            (Color::Cyan, "96"),
            (Color::White, "97"),
        ];
        for (color, expected) in colors_and_expected {
            let style = Style::new().with_fg(color);
            let ansi = style.to_ansi_start(ColorMode::TrueColor);
            assert!(
                ansi.contains(expected),
                "Color {color:?} should contain {expected}, got {ansi}"
            );
        }
    }

    #[test]
    fn test_color_to_ansi_all_dark_colors_bg() {
        let colors_and_expected: [(Color, &str); 8] = [
            (Color::Black, "40"),
            (Color::DarkRed, "41"),
            (Color::DarkGreen, "42"),
            (Color::DarkYellow, "43"),
            (Color::DarkBlue, "44"),
            (Color::DarkMagenta, "45"),
            (Color::DarkCyan, "46"),
            (Color::Grey, "47"),
        ];
        for (color, expected) in colors_and_expected {
            let style = Style::new().with_bg(color);
            let ansi = style.to_ansi_start(ColorMode::TrueColor);
            assert!(
                ansi.contains(expected),
                "Color {color:?} bg should contain {expected}, got {ansi}"
            );
        }
    }

    #[test]
    fn test_color_to_ansi_all_bright_colors_bg() {
        let colors_and_expected: [(Color, &str); 8] = [
            (Color::DarkGrey, "100"),
            (Color::Red, "101"),
            (Color::Green, "102"),
            (Color::Yellow, "103"),
            (Color::Blue, "104"),
            (Color::Magenta, "105"),
            (Color::Cyan, "106"),
            (Color::White, "107"),
        ];
        for (color, expected) in colors_and_expected {
            let style = Style::new().with_bg(color);
            let ansi = style.to_ansi_start(ColorMode::TrueColor);
            assert!(
                ansi.contains(expected),
                "Color {color:?} bg should contain {expected}, got {ansi}"
            );
        }
    }

    #[test]
    fn test_color_to_ansi_rgb_bg_256() {
        let style = Style::new().with_bg(Color::Rgb {
            r: 128,
            g: 64,
            b: 32,
        });
        let ansi = style.to_ansi_start(ColorMode::Color256);
        assert!(ansi.contains("48;5;"));
    }

    #[test]
    fn test_color_to_ansi_rgb_bg_ansi16() {
        let style = Style::new().with_bg(Color::Rgb { r: 255, g: 0, b: 0 });
        let ansi = style.to_ansi_start(ColorMode::Ansi16);
        // bg should be base + 10
        // red: (true, false, false) => base=31, bg=31+10=41
        assert!(ansi.contains("41"));
    }

    #[test]
    fn test_rgb_to_ansi16_bright_red() {
        // Bright red: r=255, g=128, b=0
        // brightness = (255+128+0)/3 = 127, bright=false (not > 127)
        // r_on=true, g_on=true(128>127), b_on=false => base=33 (yellow)
        let code = rgb_to_ansi16(255, 128, 0);
        assert_eq!(code, 33);
    }

    #[test]
    fn test_rgb_to_ansi16_bright_variant() {
        // r=200, g=200, b=0 => brightness = 133 > 127, bright=true
        // r_on=true, g_on=true, b_on=false => base=33 (yellow)
        // bright && base < 37 => 33 + 60 = 93
        let code = rgb_to_ansi16(200, 200, 0);
        assert_eq!(code, 93);
    }

    #[test]
    fn test_rgb_to_ansi16_all_on_bright() {
        // White: all on, bright (base=37), but base < 37 is false so no +60
        let code = rgb_to_ansi16(200, 200, 200);
        // brightness = 200 > 127, bright = true
        // all on => base = 37
        // bright && base < 37 is false => just base
        assert_eq!(code, 37);
    }

    #[test]
    fn test_rgb_to_256_mid_values() {
        // Mid-range values
        let idx = rgb_to_256(128, 128, 128);
        assert!((16..=231).contains(&idx));

        // Pure green
        let idx = rgb_to_256(0, 255, 0);
        assert!((16..=231).contains(&idx));

        // Pure blue
        let idx = rgb_to_256(0, 0, 255);
        assert!((16..=231).contains(&idx));
    }

    #[test]
    fn test_style_to_ansi_fg_and_bg_combined() {
        let style = Style::new()
            .with_fg(Color::Rgb { r: 255, g: 0, b: 0 })
            .with_bg(Color::Rgb { r: 0, g: 0, b: 255 })
            .bold();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("38;2;255;0;0"));
        assert!(ansi.contains("48;2;0;0;255"));
        // Bold code "1" appears at position after \x1b[
        assert!(ansi.starts_with("\x1b[1;"));
    }

    #[test]
    fn test_color_to_ansi_ansi_value_bg() {
        // AnsiValue as background
        let style = Style::new().with_bg(Color::AnsiValue(200));
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("48;5;200"));
    }

    #[test]
    fn test_color_to_ansi_reset_fg_and_bg() {
        let style = Style::new().with_fg(Color::Reset).with_bg(Color::Reset);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("39"));
        assert!(ansi.contains("49"));
    }

    #[test]
    fn test_rgb_to_ansi16_dark_blue() {
        // Dark blue: r=0, g=0, b=200
        // brightness = 200/3 = 66, not bright
        // r=false, g=false, b=true => base=34
        let code = rgb_to_ansi16(0, 0, 200);
        assert_eq!(code, 34);
    }

    #[test]
    fn test_rgb_to_ansi16_bright_blue() {
        // Bright scenario: r=0, g=128, b=255
        // brightness = (0+128+255)/3 = 127, bright=false (not > 127)
        // r=false, g=true(128>127), b=true => cyan, base=36
        let code = rgb_to_ansi16(0, 128, 255);
        assert_eq!(code, 36);
    }

    #[test]
    fn test_rgb_to_ansi16_dark_magenta() {
        // r=200, g=0, b=200
        // brightness = (200+0+200)/3 = 133, bright=true
        // r=true, g=false, b=true => base=35 (magenta)
        // bright && base < 37 => 35 + 60 = 95
        let code = rgb_to_ansi16(200, 0, 200);
        assert_eq!(code, 95);
    }

    #[test]
    fn test_rgb_to_ansi16_dark_green() {
        // r=0, g=200, b=0
        // brightness = 200/3 = 66, not bright
        // r=false, g=true, b=false => base=32
        let code = rgb_to_ansi16(0, 200, 0);
        assert_eq!(code, 32);
    }

    #[test]
    fn test_rgb_to_ansi16_bright_green() {
        // r=128, g=255, b=128
        // brightness = (128+255+128)/3 = 170, bright=true
        // r=true(128>127), g=true, b=true(128>127) => base=37 (white)
        // bright && base < 37 => false, so base=37
        let code = rgb_to_ansi16(128, 255, 128);
        assert_eq!(code, 37);
    }
}
