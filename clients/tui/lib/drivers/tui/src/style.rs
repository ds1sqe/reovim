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
}
