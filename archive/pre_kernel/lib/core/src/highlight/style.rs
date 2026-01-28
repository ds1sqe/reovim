//! Text styling for syntax highlighting
//!
//! This module provides extended ANSI support including:
//! - Standard attributes (bold, italic, underline, etc.)
//! - Extended underline styles (double, curly, dotted, dashed)
//! - Underline colors (ANSI 58;2;r;g;b)
//! - Overline and hidden text

use {
    crate::{
        constants::RESET_STYLE,
        highlight::color::{ColorMode, downgrade_color},
    },
    reovim_sys::style::Color,
};

/// Bitflags for text attributes (u16 for extended attributes)
///
/// # ANSI Codes Reference
/// - Bold: 1, Dim: 2, Italic: 3, Underline: 4, Blink: 5
/// - Reverse: 7, Hidden: 8, Strikethrough: 9
/// - Double underline: 21, Overline: 53
/// - Curly/dotted/dashed underline: 4:3, 4:4, 4:5 (Kitty/VTE extension)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attributes(u16);

impl Attributes {
    // === Standard attributes (bits 0-6) ===
    pub const BOLD: u16 = 1 << 0;
    pub const ITALIC: u16 = 1 << 1;
    pub const UNDERLINE: u16 = 1 << 2;
    pub const STRIKETHROUGH: u16 = 1 << 3;
    pub const REVERSE: u16 = 1 << 4;
    pub const BLINK: u16 = 1 << 5;
    pub const DIM: u16 = 1 << 6;

    // === Extended attributes (bits 7-15) ===
    /// Double underline (ANSI code 21)
    pub const DOUBLE_UNDERLINE: u16 = 1 << 7;
    /// Curly/wavy underline (ANSI code 4:3, Kitty/VTE extension)
    /// Commonly used for spell check errors
    pub const CURLY_UNDERLINE: u16 = 1 << 8;
    /// Dotted underline (ANSI code 4:4, Kitty/VTE extension)
    pub const DOTTED_UNDERLINE: u16 = 1 << 9;
    /// Dashed underline (ANSI code 4:5, Kitty/VTE extension)
    pub const DASHED_UNDERLINE: u16 = 1 << 10;
    /// Overline (ANSI code 53)
    pub const OVERLINE: u16 = 1 << 11;
    /// Hidden/concealed text (ANSI code 8)
    pub const HIDDEN: u16 = 1 << 12;
    // Reserved: bits 13-15 for future use

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

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if any underline variant is active
    #[must_use]
    pub const fn has_any_underline(self) -> bool {
        self.contains(Self::UNDERLINE)
            || self.contains(Self::DOUBLE_UNDERLINE)
            || self.contains(Self::CURLY_UNDERLINE)
            || self.contains(Self::DOTTED_UNDERLINE)
            || self.contains(Self::DASHED_UNDERLINE)
    }
}

/// Represents text styling properties with extended ANSI support
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub attributes: Attributes,
    /// Underline color (ANSI 58;2;r;g;b or 58;5;n)
    /// Only applies when any underline attribute is set
    pub underline_color: Option<Color>,
}

impl Style {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    // === Standard attribute builders ===

    #[must_use]
    pub fn bold(mut self) -> Self {
        self.attributes.set(Attributes::BOLD);
        self
    }

    #[must_use]
    pub fn italic(mut self) -> Self {
        self.attributes.set(Attributes::ITALIC);
        self
    }

    #[must_use]
    pub fn underline(mut self) -> Self {
        self.attributes.set(Attributes::UNDERLINE);
        self
    }

    #[must_use]
    pub fn strikethrough(mut self) -> Self {
        self.attributes.set(Attributes::STRIKETHROUGH);
        self
    }

    #[must_use]
    pub fn reverse(mut self) -> Self {
        self.attributes.set(Attributes::REVERSE);
        self
    }

    #[must_use]
    pub fn blink(mut self) -> Self {
        self.attributes.set(Attributes::BLINK);
        self
    }

    #[must_use]
    pub fn dim(mut self) -> Self {
        self.attributes.set(Attributes::DIM);
        self
    }

    // === Extended attribute builders ===

    /// Double underline (ANSI code 21)
    /// Supported by most modern terminals
    #[must_use]
    pub fn double_underline(mut self) -> Self {
        self.attributes.set(Attributes::DOUBLE_UNDERLINE);
        self
    }

    /// Curly/wavy underline (Kitty/VTE extension, code 4:3)
    /// Perfect for diagnostic errors - falls back to standard underline
    #[must_use]
    pub fn curly_underline(mut self) -> Self {
        self.attributes.set(Attributes::CURLY_UNDERLINE);
        self
    }

    /// Dotted underline (Kitty/VTE extension, code 4:4)
    /// Good for hints and suggestions - falls back to standard underline
    #[must_use]
    pub fn dotted_underline(mut self) -> Self {
        self.attributes.set(Attributes::DOTTED_UNDERLINE);
        self
    }

    /// Dashed underline (Kitty/VTE extension, code 4:5)
    /// Good for warnings - falls back to standard underline
    #[must_use]
    pub fn dashed_underline(mut self) -> Self {
        self.attributes.set(Attributes::DASHED_UNDERLINE);
        self
    }

    /// Overline (ANSI code 53)
    /// Draws a line above the text
    #[must_use]
    pub fn overline(mut self) -> Self {
        self.attributes.set(Attributes::OVERLINE);
        self
    }

    /// Hidden/concealed text (ANSI code 8)
    /// Text is invisible but takes space - useful for secrets
    #[must_use]
    pub fn hidden(mut self) -> Self {
        self.attributes.set(Attributes::HIDDEN);
        self
    }

    /// Set underline color (ANSI 58;2;r;g;b)
    /// Only visible when an underline attribute is active
    /// Supported by Kitty, iTerm2, VTE-based terminals, Windows Terminal
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn underline_color(mut self, color: Color) -> Self {
        self.underline_color = Some(color);
        self
    }

    /// Merge another style on top of this one (other takes precedence for colors)
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            fg: other.fg.or(self.fg),
            bg: other.bg.or(self.bg),
            attributes: self.attributes.union(other.attributes),
            underline_color: other.underline_color.or(self.underline_color),
        }
    }

    /// Convert to ANSI escape sequence for starting this style
    /// Color mode determines how colors are converted for terminal compatibility
    #[must_use]
    pub fn to_ansi_start(&self, color_mode: ColorMode) -> String {
        let mut codes: Vec<&str> = Vec::new();
        let mut owned_codes: Vec<String> = Vec::new();

        // Foreground color (with downgrade based on color mode)
        if let Some(fg) = &self.fg {
            let converted = downgrade_color(*fg, color_mode);
            owned_codes.push(color_to_fg_ansi(&converted));
        }

        // Background color (with downgrade based on color mode)
        // When bg is None, emit reset code (49) to clear any previous background
        if let Some(bg) = &self.bg {
            let converted = downgrade_color(*bg, color_mode);
            owned_codes.push(color_to_bg_ansi(&converted));
        } else {
            codes.push("49"); // Reset background to terminal default
        }

        // Standard attributes
        if self.attributes.contains(Attributes::BOLD) {
            codes.push("1");
        }
        if self.attributes.contains(Attributes::DIM) {
            codes.push("2");
        }
        if self.attributes.contains(Attributes::ITALIC) {
            codes.push("3");
        }
        if self.attributes.contains(Attributes::REVERSE) {
            codes.push("7");
        }
        if self.attributes.contains(Attributes::HIDDEN) {
            codes.push("8");
        }
        if self.attributes.contains(Attributes::STRIKETHROUGH) {
            codes.push("9");
        }
        if self.attributes.contains(Attributes::BLINK) {
            codes.push("5");
        }

        // Underline variants (mutually exclusive, prefer most specific)
        // Order: curly > dotted > dashed > double > standard
        if self.attributes.contains(Attributes::CURLY_UNDERLINE) {
            codes.push("4:3"); // Kitty/VTE extended underline style
        } else if self.attributes.contains(Attributes::DOTTED_UNDERLINE) {
            codes.push("4:4");
        } else if self.attributes.contains(Attributes::DASHED_UNDERLINE) {
            codes.push("4:5");
        } else if self.attributes.contains(Attributes::DOUBLE_UNDERLINE) {
            codes.push("21"); // Standard ANSI double underline
        } else if self.attributes.contains(Attributes::UNDERLINE) {
            codes.push("4");
        }

        // Overline (ANSI code 53)
        if self.attributes.contains(Attributes::OVERLINE) {
            codes.push("53");
        }

        // Underline color (ANSI code 58;2;r;g;b or 58;5;n)
        // Only emit if any underline variant is active
        if self.attributes.has_any_underline()
            && let Some(ul) = &self.underline_color
        {
            let converted = downgrade_color(*ul, color_mode);
            owned_codes.push(color_to_underline_ansi(&converted));
        }

        if codes.is_empty() && owned_codes.is_empty() {
            String::new()
        } else {
            let all_codes: Vec<&str> = owned_codes
                .iter()
                .map(String::as_str)
                .chain(codes)
                .collect();
            format!("\x1b[{}m", all_codes.join(";"))
        }
    }

    /// ANSI reset sequence
    #[must_use]
    pub const fn ansi_reset() -> &'static str {
        RESET_STYLE
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn color_to_fg_ansi(color: &Color) -> String {
    match color {
        Color::Reset => String::from("39"),
        Color::Black => String::from("30"),
        Color::DarkGrey => String::from("90"),
        Color::Red => String::from("31"),
        Color::DarkRed => String::from("91"),
        Color::Green => String::from("32"),
        Color::DarkGreen => String::from("92"),
        Color::Yellow => String::from("33"),
        Color::DarkYellow => String::from("93"),
        Color::Blue => String::from("34"),
        Color::DarkBlue => String::from("94"),
        Color::Magenta => String::from("35"),
        Color::DarkMagenta => String::from("95"),
        Color::Cyan => String::from("36"),
        Color::DarkCyan => String::from("96"),
        Color::White => String::from("37"),
        Color::Grey => String::from("97"),
        Color::AnsiValue(n) => format!("38;5;{n}"),
        Color::Rgb { r, g, b } => format!("38;2;{r};{g};{b}"),
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn color_to_bg_ansi(color: &Color) -> String {
    match color {
        Color::Reset => String::from("49"),
        Color::Black => String::from("40"),
        Color::DarkGrey => String::from("100"),
        Color::Red => String::from("41"),
        Color::DarkRed => String::from("101"),
        Color::Green => String::from("42"),
        Color::DarkGreen => String::from("102"),
        Color::Yellow => String::from("43"),
        Color::DarkYellow => String::from("103"),
        Color::Blue => String::from("44"),
        Color::DarkBlue => String::from("104"),
        Color::Magenta => String::from("45"),
        Color::DarkMagenta => String::from("105"),
        Color::Cyan => String::from("46"),
        Color::DarkCyan => String::from("106"),
        Color::White => String::from("47"),
        Color::Grey => String::from("107"),
        Color::AnsiValue(n) => format!("48;5;{n}"),
        Color::Rgb { r, g, b } => format!("48;2;{r};{g};{b}"),
    }
}

/// Convert color to underline color ANSI sequence (code 58)
/// Supported by Kitty, iTerm2, VTE-based terminals, Windows Terminal
#[allow(clippy::trivially_copy_pass_by_ref)]
fn color_to_underline_ansi(color: &Color) -> String {
    match color {
        Color::Reset => String::from("59"), // Reset underline color
        Color::AnsiValue(n) => format!("58;5;{n}"),
        Color::Rgb { r, g, b } => format!("58;2;{r};{g};{b}"),
        // For named colors, convert to ANSI 256 index
        _ => {
            let ansi = named_color_to_ansi_index(color);
            format!("58;5;{ansi}")
        }
    }
}

/// Convert named Color to ANSI 256 palette index
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn named_color_to_ansi_index(color: &Color) -> u8 {
    match color {
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
        Color::AnsiValue(n) => *n,
        // Black, Reset, and Rgb all default to 0
        Color::Black | Color::Rgb { .. } | Color::Reset => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extended_underline_ansi() {
        let style = Style::new().curly_underline();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("4:3"), "Should contain curly underline code");
    }

    #[test]
    fn test_underline_color() {
        let style = Style::new()
            .underline()
            .underline_color(Color::Rgb { r: 255, g: 0, b: 0 });
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('4'), "Should have underline");
        assert!(ansi.contains("58;2;255;0;0"), "Should have red underline color");
    }

    #[test]
    fn test_underline_color_only_with_underline() {
        // Underline color without underline attribute should not emit
        let style = Style::new().underline_color(Color::Red);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(!ansi.contains("58"), "Should not emit underline color without underline");
    }

    #[test]
    fn test_overline() {
        let style = Style::new().overline();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("53"), "Should contain overline code");
    }

    #[test]
    fn test_hidden() {
        let style = Style::new().hidden();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('8'), "Should contain hidden code");
    }

    #[test]
    fn test_merge_preserves_underline_color() {
        let base = Style::new().underline().underline_color(Color::Blue);
        let other = Style::new().bold();
        let merged = base.merge(&other);
        assert_eq!(merged.underline_color, Some(Color::Blue));
    }

    #[test]
    fn test_merge_overrides_underline_color() {
        let base = Style::new().underline().underline_color(Color::Blue);
        let other = Style::new().underline_color(Color::Red);
        let merged = base.merge(&other);
        assert_eq!(merged.underline_color, Some(Color::Red));
    }
}
