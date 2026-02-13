//! Style types for display rendering.
//!
//! These types were migrated from the legacy lib/core (now archived) to break the
//! dependency cycle. The display driver owns these types as they are
//! fundamentally about display/rendering.

use std::{fmt, str::FromStr};

use reovim_arch::Color;

// =============================================================================
// Constants
// =============================================================================

/// ANSI reset sequence.
pub const RESET_STYLE: &str = "\x1b[0m";

// =============================================================================
// ColorMode
// =============================================================================

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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn detect() -> Self {
        Self::detect_from_env(
            std::env::var("COLORTERM").ok().as_deref(),
            std::env::var("TERM").ok().as_deref(),
        )
    }

    /// Determine color mode from optional COLORTERM and TERM values.
    ///
    /// Extracted from `detect()` for testability (env var mutation is
    /// `unsafe` in Rust 2024 edition).
    #[must_use]
    pub fn detect_from_env(colorterm: Option<&str>, term: Option<&str>) -> Self {
        // Check COLORTERM first (most specific)
        if let Some(colorterm) = colorterm {
            let ct = colorterm.to_lowercase();
            if ct == "truecolor" || ct == "24bit" {
                return Self::TrueColor;
            }
        }

        // Check TERM for 256color
        if let Some(term) = term
            && term.contains("256color")
        {
            return Self::Color256;
        }

        // Fall back to basic ANSI
        Self::Ansi16
    }

    /// Parse from string for :set command.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ansi" | "ansi16" | "16" => Some(Self::Ansi16),
            "256" | "256color" => Some(Self::Color256),
            "truecolor" | "true" | "rgb" | "24bit" => Some(Self::TrueColor),
            _ => None,
        }
    }
}

// =============================================================================
// Attributes
// =============================================================================

/// Bitflags for text attributes (u16 for extended attributes).
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
    /// Double underline (ANSI code 21).
    pub const DOUBLE_UNDERLINE: u16 = 1 << 7;
    /// Curly/wavy underline (ANSI code 4:3, Kitty/VTE extension).
    pub const CURLY_UNDERLINE: u16 = 1 << 8;
    /// Dotted underline (ANSI code 4:4, Kitty/VTE extension).
    pub const DOTTED_UNDERLINE: u16 = 1 << 9;
    /// Dashed underline (ANSI code 4:5, Kitty/VTE extension).
    pub const DASHED_UNDERLINE: u16 = 1 << 10;
    /// Overline (ANSI code 53).
    pub const OVERLINE: u16 = 1 << 11;
    /// Hidden/concealed text (ANSI code 8).
    pub const HIDDEN: u16 = 1 << 12;

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

    /// Check if any underline variant is active.
    #[must_use]
    pub const fn has_any_underline(self) -> bool {
        self.contains(Self::UNDERLINE)
            || self.contains(Self::DOUBLE_UNDERLINE)
            || self.contains(Self::CURLY_UNDERLINE)
            || self.contains(Self::DOTTED_UNDERLINE)
            || self.contains(Self::DASHED_UNDERLINE)
    }
}

/// Parse comma-separated attribute names into `Attributes`.
///
/// # Supported Names
///
/// Standard: `bold`, `italic`, `underline`, `strikethrough`, `reverse`, `blink`, `dim`
/// Extended: `double_underline`, `curly_underline`, `dotted_underline`, `dashed_underline`,
///           `overline`, `hidden`
///
/// # Example
///
/// ```ignore
/// let attrs: Attributes = "bold,italic,underline".parse().unwrap();
/// assert!(attrs.contains(Attributes::BOLD));
/// ```
impl FromStr for Attributes {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut attrs = Self::new();
        for attr in s.split(',') {
            match attr.trim().to_lowercase().as_str() {
                "bold" => attrs.set(Self::BOLD),
                "italic" => attrs.set(Self::ITALIC),
                "underline" => attrs.set(Self::UNDERLINE),
                "strikethrough" => attrs.set(Self::STRIKETHROUGH),
                "reverse" => attrs.set(Self::REVERSE),
                "blink" => attrs.set(Self::BLINK),
                "dim" => attrs.set(Self::DIM),
                "double_underline" => attrs.set(Self::DOUBLE_UNDERLINE),
                "curly_underline" => attrs.set(Self::CURLY_UNDERLINE),
                "dotted_underline" => attrs.set(Self::DOTTED_UNDERLINE),
                "dashed_underline" => attrs.set(Self::DASHED_UNDERLINE),
                "overline" => attrs.set(Self::OVERLINE),
                "hidden" => attrs.set(Self::HIDDEN),
                _ => {} // Ignore unknown attributes
            }
        }
        Ok(attrs)
    }
}

/// Display attributes as comma-separated canonical names.
///
/// The output order is deterministic: standard attributes first (in bit order),
/// then extended attributes.
///
/// # Example
///
/// ```ignore
/// let attrs = Attributes::new();
/// attrs.set(Attributes::BOLD);
/// attrs.set(Attributes::ITALIC);
/// assert_eq!(attrs.to_string(), "bold,italic");
/// ```
impl fmt::Display for Attributes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();

        // Standard attributes (bits 0-6)
        if self.contains(Self::BOLD) {
            parts.push("bold");
        }
        if self.contains(Self::ITALIC) {
            parts.push("italic");
        }
        if self.contains(Self::UNDERLINE) {
            parts.push("underline");
        }
        if self.contains(Self::STRIKETHROUGH) {
            parts.push("strikethrough");
        }
        if self.contains(Self::REVERSE) {
            parts.push("reverse");
        }
        if self.contains(Self::BLINK) {
            parts.push("blink");
        }
        if self.contains(Self::DIM) {
            parts.push("dim");
        }

        // Extended attributes (bits 7-12)
        if self.contains(Self::DOUBLE_UNDERLINE) {
            parts.push("double_underline");
        }
        if self.contains(Self::CURLY_UNDERLINE) {
            parts.push("curly_underline");
        }
        if self.contains(Self::DOTTED_UNDERLINE) {
            parts.push("dotted_underline");
        }
        if self.contains(Self::DASHED_UNDERLINE) {
            parts.push("dashed_underline");
        }
        if self.contains(Self::OVERLINE) {
            parts.push("overline");
        }
        if self.contains(Self::HIDDEN) {
            parts.push("hidden");
        }

        write!(f, "{}", parts.join(","))
    }
}

// =============================================================================
// Style
// =============================================================================

/// Represents text styling properties with extended ANSI support.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub attributes: Attributes,
    /// Underline color (ANSI 58;2;r;g;b or 58;5;n).
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

    #[must_use]
    pub fn double_underline(mut self) -> Self {
        self.attributes.set(Attributes::DOUBLE_UNDERLINE);
        self
    }

    #[must_use]
    pub fn curly_underline(mut self) -> Self {
        self.attributes.set(Attributes::CURLY_UNDERLINE);
        self
    }

    #[must_use]
    pub fn dotted_underline(mut self) -> Self {
        self.attributes.set(Attributes::DOTTED_UNDERLINE);
        self
    }

    #[must_use]
    pub fn dashed_underline(mut self) -> Self {
        self.attributes.set(Attributes::DASHED_UNDERLINE);
        self
    }

    #[must_use]
    pub fn overline(mut self) -> Self {
        self.attributes.set(Attributes::OVERLINE);
        self
    }

    #[must_use]
    pub fn hidden(mut self) -> Self {
        self.attributes.set(Attributes::HIDDEN);
        self
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn underline_color(mut self, color: Color) -> Self {
        self.underline_color = Some(color);
        self
    }

    /// Merge another style on top of this one (other takes precedence for colors).
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            fg: other.fg.or(self.fg),
            bg: other.bg.or(self.bg),
            attributes: self.attributes.union(other.attributes),
            underline_color: other.underline_color.or(self.underline_color),
        }
    }

    /// Convert to ANSI escape sequence for starting this style.
    #[must_use]
    pub fn to_ansi_start(&self, color_mode: ColorMode) -> String {
        let mut codes: Vec<&str> = Vec::new();
        let mut owned_codes: Vec<String> = Vec::new();

        // Foreground color
        if let Some(fg) = &self.fg {
            let converted = downgrade_color(*fg, color_mode);
            owned_codes.push(color_to_fg_ansi(&converted));
        }

        // Background color
        if let Some(bg) = &self.bg {
            let converted = downgrade_color(*bg, color_mode);
            owned_codes.push(color_to_bg_ansi(&converted));
        } else {
            codes.push("49"); // Reset background
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

        // Underline variants
        if self.attributes.contains(Attributes::CURLY_UNDERLINE) {
            codes.push("4:3");
        } else if self.attributes.contains(Attributes::DOTTED_UNDERLINE) {
            codes.push("4:4");
        } else if self.attributes.contains(Attributes::DASHED_UNDERLINE) {
            codes.push("4:5");
        } else if self.attributes.contains(Attributes::DOUBLE_UNDERLINE) {
            codes.push("21");
        } else if self.attributes.contains(Attributes::UNDERLINE) {
            codes.push("4");
        }

        // Overline
        if self.attributes.contains(Attributes::OVERLINE) {
            codes.push("53");
        }

        // Underline color
        if self.attributes.has_any_underline()
            && let Some(ul) = &self.underline_color
        {
            let converted = downgrade_color(*ul, color_mode);
            owned_codes.push(color_to_underline_ansi(&converted));
        }

        // Always has at least one code ("49" for bg reset when bg is None),
        // so we unconditionally format the ANSI escape sequence.
        let all_codes: Vec<&str> = owned_codes
            .iter()
            .map(String::as_str)
            .chain(codes)
            .collect();
        format!("\x1b[{}m", all_codes.join(";"))
    }

    /// ANSI reset sequence.
    #[must_use]
    pub const fn ansi_reset() -> &'static str {
        RESET_STYLE
    }

    /// Build `Style` from wire format strings (RPC deserialization).
    ///
    /// This is the inverse of the JSON cell format used in `screen_content`.
    /// Uses `Color::parse()` for colors and `Attributes::from_str()` for attributes.
    ///
    /// # Wire Format
    ///
    /// The special value `"default"` means "no color specified - use terminal default".
    /// This is different from `Color::Reset` which explicitly resets styling.
    ///
    /// # Arguments
    ///
    /// * `fg` - Foreground color string (e.g., "red", "#ff0000", "ansi:196", "default")
    /// * `bg` - Background color string
    /// * `attrs` - Comma-separated attribute names (e.g., "bold,italic")
    ///
    /// # Example
    ///
    /// ```ignore
    /// let style = Style::from_wire(Some("red"), Some("#000000"), Some("bold,underline"));
    /// assert_eq!(style.fg, Some(Color::Red));
    /// assert!(style.attributes.contains(Attributes::BOLD));
    /// ```
    #[must_use]
    pub fn from_wire(fg: Option<&str>, bg: Option<&str>, attrs: Option<&str>) -> Self {
        // Helper: parse color, treating "default" as None (terminal default)
        let parse_wire_color = |s: &str| -> Option<Color> {
            if s == "default" {
                None
            } else {
                Color::parse(s)
            }
        };

        Self {
            fg: fg.and_then(parse_wire_color),
            bg: bg.and_then(parse_wire_color),
            attributes: attrs.map_or_else(Attributes::new, |s| s.parse().unwrap_or_default()),
            underline_color: None,
        }
    }
}

// =============================================================================
// Color Conversion Helpers
// =============================================================================

/// Downgrade a color to the specified mode.
#[must_use]
pub fn downgrade_color(color: Color, mode: ColorMode) -> Color {
    match (color, mode) {
        (Color::Rgb { r, g, b }, ColorMode::Color256) => Color::AnsiValue(rgb_to_ansi256(r, g, b)),
        (Color::Rgb { r, g, b }, ColorMode::Ansi16) => rgb_to_nearest_ansi16(r, g, b),
        (Color::AnsiValue(n), ColorMode::Ansi16) => ansi256_to_ansi16(n),
        _ => color,
    }
}

/// Convert RGB to nearest 256-color palette entry.
#[must_use]
pub fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    // Grayscale check
    if r == g && g == b {
        if r < 8 {
            return 16;
        }
        if r > 248 {
            return 231;
        }
        return 232 + ((r - 8) / 10).min(23);
    }

    // 6x6x6 color cube
    let to_cube = |v: u8| -> u8 {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            ((v - 35) / 40).min(5)
        }
    };

    16 + 36 * to_cube(r) + 6 * to_cube(g) + to_cube(b)
}

/// The 16 ANSI colors as (R, G, B).
const ANSI16_RGB: [(u8, u8, u8); 16] = [
    (0, 0, 0),       // 0 Black
    (128, 0, 0),     // 1 DarkRed
    (0, 128, 0),     // 2 DarkGreen
    (128, 128, 0),   // 3 DarkYellow
    (0, 0, 128),     // 4 DarkBlue
    (128, 0, 128),   // 5 DarkMagenta
    (0, 128, 128),   // 6 DarkCyan
    (192, 192, 192), // 7 Grey
    (128, 128, 128), // 8 DarkGrey
    (255, 0, 0),     // 9 Red
    (0, 255, 0),     // 10 Green
    (255, 255, 0),   // 11 Yellow
    (0, 0, 255),     // 12 Blue
    (255, 0, 255),   // 13 Magenta
    (0, 255, 255),   // 14 Cyan
    (255, 255, 255), // 15 White
];

/// Find nearest ANSI 16 color using squared Euclidean distance.
#[allow(clippy::cast_possible_truncation)]
fn rgb_to_nearest_ansi16(r: u8, g: u8, b: u8) -> Color {
    let mut best_idx = 0u8;
    let mut best_dist = u32::MAX;

    for (idx, &(ar, ag, ab)) in ANSI16_RGB.iter().enumerate() {
        let dr = (i32::from(r) - i32::from(ar)).unsigned_abs();
        let dg = (i32::from(g) - i32::from(ag)).unsigned_abs();
        let db = (i32::from(b) - i32::from(ab)).unsigned_abs();
        let dist = dr * dr + dg * dg + db * db;

        if dist < best_dist {
            best_dist = dist;
            best_idx = idx as u8;
        }
    }

    ansi_index_to_color(best_idx)
}

/// Convert ANSI 256 index to ANSI 16.
fn ansi256_to_ansi16(n: u8) -> Color {
    if n < 16 {
        ansi_index_to_color(n)
    } else if n < 232 {
        // Color cube - convert to RGB then to nearest ANSI 16
        let idx = n - 16;
        let r_idx = (idx / 36) % 6;
        let g_idx = (idx / 6) % 6;
        let b_idx = idx % 6;
        let to_rgb = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        rgb_to_nearest_ansi16(to_rgb(r_idx), to_rgb(g_idx), to_rgb(b_idx))
    } else {
        // Grayscale
        let gray = 8 + (n - 232) * 10;
        rgb_to_nearest_ansi16(gray, gray, gray)
    }
}

/// Convert index 0-15 to Color enum.
const fn ansi_index_to_color(idx: u8) -> Color {
    match idx {
        0 => Color::Black,
        1 => Color::DarkRed,
        2 => Color::DarkGreen,
        3 => Color::DarkYellow,
        4 => Color::DarkBlue,
        5 => Color::DarkMagenta,
        6 => Color::DarkCyan,
        7 => Color::Grey,
        8 => Color::DarkGrey,
        9 => Color::Red,
        10 => Color::Green,
        11 => Color::Yellow,
        12 => Color::Blue,
        13 => Color::Magenta,
        14 => Color::Cyan,
        15 => Color::White,
        _ => Color::Reset,
    }
}

// =============================================================================
// ANSI Code Generation
// =============================================================================

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

#[allow(clippy::trivially_copy_pass_by_ref)]
fn color_to_underline_ansi(color: &Color) -> String {
    match color {
        Color::Reset => String::from("59"),
        Color::AnsiValue(n) => format!("58;5;{n}"),
        Color::Rgb { r, g, b } => format!("58;2;{r};{g};{b}"),
        _ => {
            let ansi = named_color_to_ansi_index(color);
            format!("58;5;{ansi}")
        }
    }
}

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
        Color::Black | Color::Rgb { .. } | Color::Reset => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colormode_parse() {
        assert_eq!(ColorMode::parse("ansi"), Some(ColorMode::Ansi16));
        assert_eq!(ColorMode::parse("256"), Some(ColorMode::Color256));
        assert_eq!(ColorMode::parse("truecolor"), Some(ColorMode::TrueColor));
        assert_eq!(ColorMode::parse("invalid"), None);
    }

    #[test]
    fn test_style_builder() {
        let style = Style::new().fg(Color::Red).bold().underline();
        assert_eq!(style.fg, Some(Color::Red));
        assert!(style.attributes.contains(Attributes::BOLD));
        assert!(style.attributes.contains(Attributes::UNDERLINE));
    }

    #[test]
    fn test_style_merge() {
        let base = Style::new().fg(Color::Red).bold();
        let overlay = Style::new().bg(Color::Blue).italic();
        let merged = base.merge(&overlay);

        assert_eq!(merged.fg, Some(Color::Red));
        assert_eq!(merged.bg, Some(Color::Blue));
        assert!(merged.attributes.contains(Attributes::BOLD));
        assert!(merged.attributes.contains(Attributes::ITALIC));
    }

    #[test]
    fn test_downgrade_truecolor() {
        let rgb = Color::Rgb { r: 255, g: 0, b: 0 };
        assert_eq!(downgrade_color(rgb, ColorMode::TrueColor), rgb);
    }

    #[test]
    fn test_downgrade_to_256() {
        let rgb = Color::Rgb { r: 255, g: 0, b: 0 };
        let converted = downgrade_color(rgb, ColorMode::Color256);
        assert!(matches!(converted, Color::AnsiValue(_)));
    }

    // =========================================================================
    // Attributes FromStr / Display tests
    // =========================================================================

    #[test]
    fn test_attributes_from_str_single() {
        let attrs: Attributes = "bold".parse().unwrap();
        assert!(attrs.contains(Attributes::BOLD));
        assert!(!attrs.contains(Attributes::ITALIC));
    }

    #[test]
    fn test_attributes_from_str_multiple() {
        let attrs: Attributes = "bold,italic,underline".parse().unwrap();
        assert!(attrs.contains(Attributes::BOLD));
        assert!(attrs.contains(Attributes::ITALIC));
        assert!(attrs.contains(Attributes::UNDERLINE));
        assert!(!attrs.contains(Attributes::REVERSE));
    }

    #[test]
    fn test_attributes_from_str_case_insensitive() {
        let attrs: Attributes = "BOLD,Italic,UNDERLINE".parse().unwrap();
        assert!(attrs.contains(Attributes::BOLD));
        assert!(attrs.contains(Attributes::ITALIC));
        assert!(attrs.contains(Attributes::UNDERLINE));
    }

    #[test]
    fn test_attributes_from_str_with_spaces() {
        let attrs: Attributes = "bold , italic , underline".parse().unwrap();
        assert!(attrs.contains(Attributes::BOLD));
        assert!(attrs.contains(Attributes::ITALIC));
        assert!(attrs.contains(Attributes::UNDERLINE));
    }

    #[test]
    fn test_attributes_from_str_extended() {
        let attrs: Attributes = "curly_underline,overline,hidden".parse().unwrap();
        assert!(attrs.contains(Attributes::CURLY_UNDERLINE));
        assert!(attrs.contains(Attributes::OVERLINE));
        assert!(attrs.contains(Attributes::HIDDEN));
    }

    #[test]
    fn test_attributes_from_str_unknown_ignored() {
        let attrs: Attributes = "bold,foobar,italic".parse().unwrap();
        assert!(attrs.contains(Attributes::BOLD));
        assert!(attrs.contains(Attributes::ITALIC));
    }

    #[test]
    fn test_attributes_display_single() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::BOLD);
        assert_eq!(attrs.to_string(), "bold");
    }

    #[test]
    fn test_attributes_display_multiple() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::BOLD);
        attrs.set(Attributes::ITALIC);
        attrs.set(Attributes::UNDERLINE);
        assert_eq!(attrs.to_string(), "bold,italic,underline");
    }

    #[test]
    fn test_attributes_display_empty() {
        let attrs = Attributes::new();
        assert_eq!(attrs.to_string(), "");
    }

    #[test]
    fn test_attributes_roundtrip() {
        // Test that parse(to_string()) == original
        let mut original = Attributes::new();
        original.set(Attributes::BOLD);
        original.set(Attributes::ITALIC);
        original.set(Attributes::CURLY_UNDERLINE);

        let s = original.to_string();
        let parsed: Attributes = s.parse().unwrap();
        assert_eq!(parsed.0, original.0);
    }

    // =========================================================================
    // Style::from_wire tests
    // =========================================================================

    #[test]
    fn test_style_from_wire_full() {
        let style = Style::from_wire(Some("red"), Some("#000000"), Some("bold,italic"));
        assert_eq!(style.fg, Some(Color::Red));
        assert_eq!(style.bg, Some(Color::Rgb { r: 0, g: 0, b: 0 }));
        assert!(style.attributes.contains(Attributes::BOLD));
        assert!(style.attributes.contains(Attributes::ITALIC));
    }

    #[test]
    fn test_style_from_wire_colors_only() {
        let style = Style::from_wire(Some("blue"), Some("ansi:196"), None);
        assert_eq!(style.fg, Some(Color::Blue));
        assert_eq!(style.bg, Some(Color::AnsiValue(196)));
        assert_eq!(style.attributes.0, 0);
    }

    #[test]
    fn test_style_from_wire_none() {
        let style = Style::from_wire(None, None, None);
        assert!(style.fg.is_none());
        assert!(style.bg.is_none());
        assert_eq!(style.attributes.0, 0);
    }

    #[test]
    fn test_style_from_wire_default_color() {
        // "default" means no color (uses terminal default)
        let style = Style::from_wire(Some("default"), Some("default"), None);
        assert!(style.fg.is_none());
        assert!(style.bg.is_none());
    }

    #[test]
    fn test_style_from_wire_hex_colors() {
        let style = Style::from_wire(Some("#ff5500"), Some("#003366"), None);
        assert_eq!(
            style.fg,
            Some(Color::Rgb {
                r: 255,
                g: 85,
                b: 0
            })
        );
        assert_eq!(
            style.bg,
            Some(Color::Rgb {
                r: 0,
                g: 51,
                b: 102
            })
        );
    }

    // =========================================================================
    // Extended Attributes tests
    // =========================================================================

    #[test]
    fn test_attributes_from_str_all_standard() {
        let attrs: Attributes = "bold,italic,underline,strikethrough,reverse,blink,dim"
            .parse()
            .unwrap();
        assert!(attrs.contains(Attributes::BOLD));
        assert!(attrs.contains(Attributes::ITALIC));
        assert!(attrs.contains(Attributes::UNDERLINE));
        assert!(attrs.contains(Attributes::STRIKETHROUGH));
        assert!(attrs.contains(Attributes::REVERSE));
        assert!(attrs.contains(Attributes::BLINK));
        assert!(attrs.contains(Attributes::DIM));
    }

    #[test]
    fn test_attributes_from_str_all_extended() {
        let attrs: Attributes =
            "double_underline,curly_underline,dotted_underline,dashed_underline,overline,hidden"
                .parse()
                .unwrap();
        assert!(attrs.contains(Attributes::DOUBLE_UNDERLINE));
        assert!(attrs.contains(Attributes::CURLY_UNDERLINE));
        assert!(attrs.contains(Attributes::DOTTED_UNDERLINE));
        assert!(attrs.contains(Attributes::DASHED_UNDERLINE));
        assert!(attrs.contains(Attributes::OVERLINE));
        assert!(attrs.contains(Attributes::HIDDEN));
    }

    #[test]
    fn test_attributes_display_extended() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::DOUBLE_UNDERLINE);
        attrs.set(Attributes::OVERLINE);
        attrs.set(Attributes::HIDDEN);
        assert_eq!(attrs.to_string(), "double_underline,overline,hidden");
    }

    #[test]
    fn test_attributes_display_all_standard() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::BOLD);
        attrs.set(Attributes::ITALIC);
        attrs.set(Attributes::UNDERLINE);
        attrs.set(Attributes::STRIKETHROUGH);
        attrs.set(Attributes::REVERSE);
        attrs.set(Attributes::BLINK);
        attrs.set(Attributes::DIM);
        assert_eq!(attrs.to_string(), "bold,italic,underline,strikethrough,reverse,blink,dim");
    }

    #[test]
    fn test_attributes_has_any_underline() {
        let mut attrs = Attributes::new();
        assert!(!attrs.has_any_underline());

        attrs.set(Attributes::UNDERLINE);
        assert!(attrs.has_any_underline());

        let mut attrs2 = Attributes::new();
        attrs2.set(Attributes::DOUBLE_UNDERLINE);
        assert!(attrs2.has_any_underline());

        let mut attrs3 = Attributes::new();
        attrs3.set(Attributes::CURLY_UNDERLINE);
        assert!(attrs3.has_any_underline());

        let mut attrs4 = Attributes::new();
        attrs4.set(Attributes::DOTTED_UNDERLINE);
        assert!(attrs4.has_any_underline());

        let mut attrs5 = Attributes::new();
        attrs5.set(Attributes::DASHED_UNDERLINE);
        assert!(attrs5.has_any_underline());
    }

    #[test]
    fn test_attributes_union() {
        let mut a = Attributes::new();
        a.set(Attributes::BOLD);
        let mut b = Attributes::new();
        b.set(Attributes::ITALIC);
        let c = a.union(b);
        assert!(c.contains(Attributes::BOLD));
        assert!(c.contains(Attributes::ITALIC));
    }

    // =========================================================================
    // Style builder tests for extended attributes
    // =========================================================================

    #[test]
    fn test_style_builder_all_attributes() {
        let style = Style::new()
            .bold()
            .italic()
            .underline()
            .strikethrough()
            .reverse()
            .blink()
            .dim()
            .double_underline()
            .curly_underline()
            .dotted_underline()
            .dashed_underline()
            .overline()
            .hidden();

        assert!(style.attributes.contains(Attributes::BOLD));
        assert!(style.attributes.contains(Attributes::ITALIC));
        assert!(style.attributes.contains(Attributes::UNDERLINE));
        assert!(style.attributes.contains(Attributes::STRIKETHROUGH));
        assert!(style.attributes.contains(Attributes::REVERSE));
        assert!(style.attributes.contains(Attributes::BLINK));
        assert!(style.attributes.contains(Attributes::DIM));
        assert!(style.attributes.contains(Attributes::DOUBLE_UNDERLINE));
        assert!(style.attributes.contains(Attributes::CURLY_UNDERLINE));
        assert!(style.attributes.contains(Attributes::DOTTED_UNDERLINE));
        assert!(style.attributes.contains(Attributes::DASHED_UNDERLINE));
        assert!(style.attributes.contains(Attributes::OVERLINE));
        assert!(style.attributes.contains(Attributes::HIDDEN));
    }

    #[test]
    fn test_style_underline_color() {
        let style = Style::new().underline_color(Color::Red);
        assert_eq!(style.underline_color, Some(Color::Red));
    }

    #[test]
    fn test_style_merge_underline_color() {
        let base = Style::new().underline_color(Color::Red);
        let overlay = Style::new().underline_color(Color::Blue);
        let merged = base.merge(&overlay);
        assert_eq!(merged.underline_color, Some(Color::Blue));

        // overlay without underline_color should keep base
        let overlay2 = Style::new();
        let merged2 = base.merge(&overlay2);
        assert_eq!(merged2.underline_color, Some(Color::Red));
    }

    #[test]
    fn test_style_merge_fg_override() {
        let base = Style::new().fg(Color::Red);
        let overlay = Style::new().fg(Color::Blue);
        let merged = base.merge(&overlay);
        assert_eq!(merged.fg, Some(Color::Blue));
    }

    // =========================================================================
    // ANSI output tests
    // =========================================================================

    #[test]
    fn test_ansi_reset() {
        assert_eq!(Style::ansi_reset(), "\x1b[0m");
    }

    #[test]
    fn test_style_to_ansi_empty() {
        let style = Style::new();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        // Empty style with no fg, no bg should have the bg reset code "49"
        assert!(ansi.contains("49"));
    }

    #[test]
    fn test_style_to_ansi_fg_bg() {
        let style = Style::new()
            .fg(Color::Rgb { r: 255, g: 0, b: 0 })
            .bg(Color::Rgb { r: 0, g: 0, b: 255 });
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("38;2;255;0;0")); // fg RGB
        assert!(ansi.contains("48;2;0;0;255")); // bg RGB
    }

    #[test]
    fn test_style_to_ansi_bold_italic() {
        let style = Style::new().bold().italic();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('1')); // bold
        assert!(ansi.contains('3')); // italic
    }

    #[test]
    fn test_style_to_ansi_all_standard_attrs() {
        let style = Style::new()
            .bold()
            .dim()
            .italic()
            .reverse()
            .hidden()
            .strikethrough()
            .blink();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('1')); // bold
        assert!(ansi.contains('2')); // dim
        assert!(ansi.contains('3')); // italic
        assert!(ansi.contains('7')); // reverse
        assert!(ansi.contains('8')); // hidden
        assert!(ansi.contains('9')); // strikethrough
        assert!(ansi.contains('5')); // blink
    }

    #[test]
    fn test_style_to_ansi_curly_underline() {
        let style = Style::new().curly_underline();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("4:3"));
    }

    #[test]
    fn test_style_to_ansi_dotted_underline() {
        let style = Style::new().dotted_underline();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("4:4"));
    }

    #[test]
    fn test_style_to_ansi_dashed_underline() {
        let style = Style::new().dashed_underline();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("4:5"));
    }

    #[test]
    fn test_style_to_ansi_double_underline() {
        let style = Style::new().double_underline();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("21"));
    }

    #[test]
    fn test_style_to_ansi_plain_underline() {
        let style = Style::new().underline();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains('4'));
    }

    #[test]
    fn test_style_to_ansi_overline() {
        let style = Style::new().overline();
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("53"));
    }

    #[test]
    fn test_style_to_ansi_underline_color() {
        let style = Style::new().underline().underline_color(Color::Rgb {
            r: 255,
            g: 128,
            b: 0,
        });
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("58;2;255;128;0"));
    }

    #[test]
    fn test_style_to_ansi_underline_color_ansi256() {
        let style = Style::new()
            .underline()
            .underline_color(Color::AnsiValue(196));
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("58;5;196"));
    }

    #[test]
    fn test_style_to_ansi_underline_color_named() {
        let style = Style::new().underline().underline_color(Color::Red);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        // Named colors get converted via named_color_to_ansi_index
        assert!(ansi.contains("58;5;9")); // Red = index 9
    }

    #[test]
    fn test_style_to_ansi_underline_color_reset() {
        let style = Style::new().underline().underline_color(Color::Reset);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("59")); // underline color reset
    }

    #[test]
    fn test_style_to_ansi_no_underline_color_without_underline() {
        // Underline color should NOT be emitted if no underline attribute is set
        let style = Style::new().underline_color(Color::Red);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(!ansi.contains("58"));
    }

    #[test]
    fn test_style_to_ansi_named_fg_colors() {
        let colors = [
            (Color::Reset, "39"),
            (Color::Black, "30"),
            (Color::DarkGrey, "90"),
            (Color::Red, "31"),
            (Color::DarkRed, "91"),
            (Color::Green, "32"),
            (Color::DarkGreen, "92"),
            (Color::Yellow, "33"),
            (Color::DarkYellow, "93"),
            (Color::Blue, "34"),
            (Color::DarkBlue, "94"),
            (Color::Magenta, "35"),
            (Color::DarkMagenta, "95"),
            (Color::Cyan, "36"),
            (Color::DarkCyan, "96"),
            (Color::White, "37"),
            (Color::Grey, "97"),
        ];

        for (color, expected_code) in colors {
            let style = Style::new().fg(color);
            let ansi = style.to_ansi_start(ColorMode::TrueColor);
            assert!(
                ansi.contains(expected_code),
                "fg color {color:?} should produce code {expected_code}, got: {ansi}"
            );
        }
    }

    #[test]
    fn test_style_to_ansi_named_bg_colors() {
        let colors = [
            (Color::Reset, "49"),
            (Color::Black, "40"),
            (Color::DarkGrey, "100"),
            (Color::Red, "41"),
            (Color::DarkRed, "101"),
            (Color::Green, "42"),
            (Color::DarkGreen, "102"),
            (Color::Yellow, "43"),
            (Color::DarkYellow, "103"),
            (Color::Blue, "44"),
            (Color::DarkBlue, "104"),
            (Color::Magenta, "45"),
            (Color::DarkMagenta, "105"),
            (Color::Cyan, "46"),
            (Color::DarkCyan, "106"),
            (Color::White, "47"),
            (Color::Grey, "107"),
        ];

        for (color, expected_code) in colors {
            let style = Style::new().bg(color);
            let ansi = style.to_ansi_start(ColorMode::TrueColor);
            assert!(
                ansi.contains(expected_code),
                "bg color {color:?} should produce code {expected_code}, got: {ansi}"
            );
        }
    }

    #[test]
    fn test_style_to_ansi_ansi256_fg() {
        let style = Style::new().fg(Color::AnsiValue(196));
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("38;5;196"));
    }

    #[test]
    fn test_style_to_ansi_ansi256_bg() {
        let style = Style::new().bg(Color::AnsiValue(52));
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(ansi.contains("48;5;52"));
    }

    // =========================================================================
    // Color downgrade tests
    // =========================================================================

    #[test]
    fn test_downgrade_rgb_to_ansi16() {
        let rgb = Color::Rgb { r: 255, g: 0, b: 0 };
        let converted = downgrade_color(rgb, ColorMode::Ansi16);
        // Pure red should map to Red (index 9)
        assert_eq!(converted, Color::Red);
    }

    #[test]
    fn test_downgrade_ansi256_to_ansi16() {
        // Index 0-15 should map to named colors
        let c = downgrade_color(Color::AnsiValue(0), ColorMode::Ansi16);
        assert_eq!(c, Color::Black);

        let c = downgrade_color(Color::AnsiValue(15), ColorMode::Ansi16);
        assert_eq!(c, Color::White);

        // Color cube (16-231) should map to nearest ANSI 16
        let c = downgrade_color(Color::AnsiValue(196), ColorMode::Ansi16);
        // 196 is bright red in 6x6x6 cube
        assert_eq!(c, Color::Red);

        // Grayscale (232-255) should map to nearest ANSI 16
        let c = downgrade_color(Color::AnsiValue(232), ColorMode::Ansi16);
        // 232 is very dark gray -> should map to Black
        assert_eq!(c, Color::Black);

        let c = downgrade_color(Color::AnsiValue(255), ColorMode::Ansi16);
        // 255 is near-white gray -> should map to White
        assert_eq!(c, Color::White);
    }

    #[test]
    fn test_downgrade_named_to_ansi16_noop() {
        // Named colors should pass through unchanged
        assert_eq!(downgrade_color(Color::Red, ColorMode::Ansi16), Color::Red);
    }

    #[test]
    fn test_downgrade_noop_for_same_or_higher_mode() {
        let ansi = Color::AnsiValue(100);
        // AnsiValue to 256 should be noop
        assert_eq!(downgrade_color(ansi, ColorMode::Color256), ansi);
        // AnsiValue to TrueColor should be noop
        assert_eq!(downgrade_color(ansi, ColorMode::TrueColor), ansi);
        // Named color stays as-is
        assert_eq!(downgrade_color(Color::Red, ColorMode::TrueColor), Color::Red);
    }

    // =========================================================================
    // rgb_to_ansi256 tests
    // =========================================================================

    #[test]
    fn test_rgb_to_ansi256_grayscale() {
        // Pure black -> 16
        assert_eq!(rgb_to_ansi256(0, 0, 0), 16);
        // Near-white -> 231
        assert_eq!(rgb_to_ansi256(255, 255, 255), 231);
        // Mid gray -> grayscale range
        let mid = rgb_to_ansi256(128, 128, 128);
        assert!(mid >= 232);
    }

    #[test]
    fn test_rgb_to_ansi256_color_cube() {
        // Pure red -> color cube
        let red = rgb_to_ansi256(255, 0, 0);
        assert!((16..232).contains(&red));
    }

    // =========================================================================
    // ColorMode parse extended
    // =========================================================================

    #[test]
    fn test_colormode_parse_all_variants() {
        assert_eq!(ColorMode::parse("ansi16"), Some(ColorMode::Ansi16));
        assert_eq!(ColorMode::parse("16"), Some(ColorMode::Ansi16));
        assert_eq!(ColorMode::parse("256color"), Some(ColorMode::Color256));
        assert_eq!(ColorMode::parse("true"), Some(ColorMode::TrueColor));
        assert_eq!(ColorMode::parse("rgb"), Some(ColorMode::TrueColor));
        assert_eq!(ColorMode::parse("24bit"), Some(ColorMode::TrueColor));
        assert_eq!(ColorMode::parse("TRUECOLOR"), Some(ColorMode::TrueColor));
    }

    #[test]
    fn test_colormode_default() {
        assert_eq!(ColorMode::default(), ColorMode::TrueColor);
    }

    // =========================================================================
    // Style::from_wire extended tests
    // =========================================================================

    #[test]
    fn test_style_from_wire_attrs_only() {
        let style = Style::from_wire(None, None, Some("bold,underline"));
        assert!(style.fg.is_none());
        assert!(style.bg.is_none());
        assert!(style.attributes.contains(Attributes::BOLD));
        assert!(style.attributes.contains(Attributes::UNDERLINE));
    }

    #[test]
    fn test_style_to_ansi_256_mode_downgrade() {
        let style = Style::new()
            .fg(Color::Rgb { r: 255, g: 0, b: 0 })
            .bg(Color::Rgb { r: 0, g: 255, b: 0 });
        let ansi = style.to_ansi_start(ColorMode::Color256);
        // In 256 mode, RGB colors are downgraded to 256 palette
        assert!(ansi.contains("38;5;")); // fg 256-color
        assert!(ansi.contains("48;5;")); // bg 256-color
    }

    #[test]
    fn test_named_color_to_ansi_index() {
        assert_eq!(named_color_to_ansi_index(&Color::Black), 0);
        assert_eq!(named_color_to_ansi_index(&Color::DarkRed), 1);
        assert_eq!(named_color_to_ansi_index(&Color::DarkGreen), 2);
        assert_eq!(named_color_to_ansi_index(&Color::DarkYellow), 3);
        assert_eq!(named_color_to_ansi_index(&Color::DarkBlue), 4);
        assert_eq!(named_color_to_ansi_index(&Color::DarkMagenta), 5);
        assert_eq!(named_color_to_ansi_index(&Color::DarkCyan), 6);
        assert_eq!(named_color_to_ansi_index(&Color::Grey), 7);
        assert_eq!(named_color_to_ansi_index(&Color::DarkGrey), 8);
        assert_eq!(named_color_to_ansi_index(&Color::Red), 9);
        assert_eq!(named_color_to_ansi_index(&Color::Green), 10);
        assert_eq!(named_color_to_ansi_index(&Color::Yellow), 11);
        assert_eq!(named_color_to_ansi_index(&Color::Blue), 12);
        assert_eq!(named_color_to_ansi_index(&Color::Magenta), 13);
        assert_eq!(named_color_to_ansi_index(&Color::Cyan), 14);
        assert_eq!(named_color_to_ansi_index(&Color::White), 15);
        assert_eq!(named_color_to_ansi_index(&Color::Reset), 0);
    }

    // =========================================================================
    // named_color_to_ansi_index: AnsiValue passthrough (line 718)
    // =========================================================================

    #[test]
    fn test_named_color_to_ansi_index_ansi_value() {
        assert_eq!(named_color_to_ansi_index(&Color::AnsiValue(42)), 42);
        assert_eq!(named_color_to_ansi_index(&Color::AnsiValue(0)), 0);
        assert_eq!(named_color_to_ansi_index(&Color::AnsiValue(255)), 255);
    }

    // =========================================================================
    // Attributes Display: dotted_underline and dashed_underline (lines 227, 230)
    // =========================================================================

    #[test]
    fn test_attributes_display_dotted_underline() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::DOTTED_UNDERLINE);
        assert_eq!(attrs.to_string(), "dotted_underline");
    }

    #[test]
    fn test_attributes_display_dashed_underline() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::DASHED_UNDERLINE);
        assert_eq!(attrs.to_string(), "dashed_underline");
    }

    #[test]
    fn test_attributes_display_all_extended() {
        let mut attrs = Attributes::new();
        attrs.set(Attributes::DOUBLE_UNDERLINE);
        attrs.set(Attributes::CURLY_UNDERLINE);
        attrs.set(Attributes::DOTTED_UNDERLINE);
        attrs.set(Attributes::DASHED_UNDERLINE);
        attrs.set(Attributes::OVERLINE);
        attrs.set(Attributes::HIDDEN);
        assert_eq!(
            attrs.to_string(),
            "double_underline,curly_underline,dotted_underline,dashed_underline,overline,hidden"
        );
    }

    // =========================================================================
    // rgb_to_ansi256: to_cube returning 1 for v in [48, 115) (line 541)
    // =========================================================================

    #[test]
    fn test_rgb_to_ansi256_cube_index_1() {
        // r=80 is in [48, 115) so to_cube(80) = 1, g=0 and b=0 are < 48 so to_cube = 0
        // Not grayscale because r != g
        // Result: 16 + 36*1 + 6*0 + 0 = 52
        assert_eq!(rgb_to_ansi256(80, 0, 0), 52);
    }

    #[test]
    fn test_rgb_to_ansi256_cube_index_1_green() {
        // g=100 is in [48, 115), r and b are different
        // to_cube(0) = 0, to_cube(100) = 1, to_cube(0) = 0
        // Result: 16 + 36*0 + 6*1 + 0 = 22
        assert_eq!(rgb_to_ansi256(0, 100, 0), 22);
    }

    #[test]
    fn test_rgb_to_ansi256_cube_index_1_blue() {
        // b=60 is in [48, 115)
        // to_cube(0) = 0, to_cube(0) = 0, to_cube(60) = 1
        // Result: 16 + 36*0 + 6*0 + 1 = 17
        assert_eq!(rgb_to_ansi256(0, 0, 60), 17);
    }

    // =========================================================================
    // ansi_index_to_color: all 16 indices (lines 614-629)
    // =========================================================================

    #[test]
    fn test_ansi256_to_ansi16_all_basic_indices() {
        // Exercise ansi256_to_ansi16 with indices 0-15 which directly call
        // ansi_index_to_color, covering all match arms in lines 612-631.
        let expected = [
            (0, Color::Black),
            (1, Color::DarkRed),
            (2, Color::DarkGreen),
            (3, Color::DarkYellow),
            (4, Color::DarkBlue),
            (5, Color::DarkMagenta),
            (6, Color::DarkCyan),
            (7, Color::Grey),
            (8, Color::DarkGrey),
            (9, Color::Red),
            (10, Color::Green),
            (11, Color::Yellow),
            (12, Color::Blue),
            (13, Color::Magenta),
            (14, Color::Cyan),
            (15, Color::White),
        ];

        for (idx, expected_color) in expected {
            let result = ansi256_to_ansi16(idx);
            assert_eq!(
                result, expected_color,
                "ansi256_to_ansi16({idx}) should be {expected_color:?}, got {result:?}"
            );
        }
    }

    #[test]
    fn test_ansi_index_to_color_out_of_range() {
        // Index >= 16 should map to Color::Reset (line 629)
        assert_eq!(ansi_index_to_color(16), Color::Reset);
        assert_eq!(ansi_index_to_color(255), Color::Reset);
    }

    // =========================================================================
    // ColorMode::detect_from_env() (covers lines 43-44, 47-48, 50-51, 54)
    // =========================================================================

    #[test]
    fn test_colormode_detect_truecolor() {
        assert_eq!(ColorMode::detect_from_env(Some("truecolor"), None), ColorMode::TrueColor,);
    }

    #[test]
    fn test_colormode_detect_truecolor_uppercase() {
        assert_eq!(ColorMode::detect_from_env(Some("TRUECOLOR"), None), ColorMode::TrueColor,);
    }

    #[test]
    fn test_colormode_detect_24bit() {
        assert_eq!(ColorMode::detect_from_env(Some("24bit"), None), ColorMode::TrueColor,);
    }

    #[test]
    fn test_colormode_detect_256color() {
        assert_eq!(ColorMode::detect_from_env(None, Some("xterm-256color")), ColorMode::Color256,);
    }

    #[test]
    fn test_colormode_detect_256color_screen() {
        assert_eq!(ColorMode::detect_from_env(None, Some("screen-256color")), ColorMode::Color256,);
    }

    #[test]
    fn test_colormode_detect_ansi16_fallback() {
        assert_eq!(ColorMode::detect_from_env(None, Some("xterm")), ColorMode::Ansi16,);
    }

    #[test]
    fn test_colormode_detect_no_env_vars() {
        assert_eq!(ColorMode::detect_from_env(None, None), ColorMode::Ansi16,);
    }

    #[test]
    fn test_colormode_detect_colorterm_non_truecolor_with_256_term() {
        // COLORTERM set but not "truecolor" or "24bit" -> falls through to TERM check
        assert_eq!(
            ColorMode::detect_from_env(Some("something-else"), Some("xterm-256color")),
            ColorMode::Color256,
        );
    }

    #[test]
    fn test_colormode_detect_colorterm_non_truecolor_no_term() {
        // COLORTERM set to unrecognized value, no TERM -> fallback to Ansi16
        assert_eq!(ColorMode::detect_from_env(Some("something-else"), None), ColorMode::Ansi16,);
    }

    #[test]
    fn test_colormode_detect_truecolor_overrides_256_term() {
        // COLORTERM=truecolor should take precedence even if TERM has 256color
        assert_eq!(
            ColorMode::detect_from_env(Some("truecolor"), Some("xterm-256color")),
            ColorMode::TrueColor,
        );
    }

    // =========================================================================
    // to_ansi_start: empty output path (line 447)
    // =========================================================================
    //
    // Line 447 (String::new() when both codes and owned_codes are empty) is
    // unreachable with the current logic: when bg is None, "49" is always
    // pushed to codes; when bg is Some, the bg ANSI code is pushed to
    // owned_codes. So at least one collection always has an element.
    // No test needed for dead code.

    // =========================================================================
    // Underline color with named color via underline_ansi (exercises
    // named_color_to_ansi_index through color_to_underline_ansi)
    // =========================================================================

    #[test]
    fn test_underline_color_named_all_variants() {
        // Test that all named colors produce valid underline ANSI via
        // the named_color_to_ansi_index -> format!("58;5;{ansi}") path
        let named_colors = [
            (Color::DarkRed, "58;5;1"),
            (Color::DarkGreen, "58;5;2"),
            (Color::DarkYellow, "58;5;3"),
            (Color::DarkBlue, "58;5;4"),
            (Color::DarkMagenta, "58;5;5"),
            (Color::DarkCyan, "58;5;6"),
            (Color::Grey, "58;5;7"),
            (Color::DarkGrey, "58;5;8"),
            (Color::Green, "58;5;10"),
            (Color::Yellow, "58;5;11"),
            (Color::Blue, "58;5;12"),
            (Color::Magenta, "58;5;13"),
            (Color::Cyan, "58;5;14"),
            (Color::White, "58;5;15"),
            (Color::Black, "58;5;0"),
        ];

        for (color, expected_code) in named_colors {
            let style = Style::new().underline().underline_color(color);
            let ansi = style.to_ansi_start(ColorMode::TrueColor);
            assert!(
                ansi.contains(expected_code),
                "underline color {color:?} should produce {expected_code}, got: {ansi}"
            );
        }
    }

    #[test]
    fn test_color_to_fg_ansi_direct_all() {
        assert_eq!(color_to_fg_ansi(&Color::Reset), "39");
        assert_eq!(color_to_fg_ansi(&Color::Black), "30");
        assert_eq!(color_to_fg_ansi(&Color::DarkGrey), "90");
        assert_eq!(color_to_fg_ansi(&Color::Red), "31");
        assert_eq!(color_to_fg_ansi(&Color::DarkRed), "91");
        assert_eq!(color_to_fg_ansi(&Color::Green), "32");
        assert_eq!(color_to_fg_ansi(&Color::DarkGreen), "92");
        assert_eq!(color_to_fg_ansi(&Color::Yellow), "33");
        assert_eq!(color_to_fg_ansi(&Color::DarkYellow), "93");
        assert_eq!(color_to_fg_ansi(&Color::Blue), "34");
        assert_eq!(color_to_fg_ansi(&Color::DarkBlue), "94");
        assert_eq!(color_to_fg_ansi(&Color::Magenta), "35");
        assert_eq!(color_to_fg_ansi(&Color::DarkMagenta), "95");
        assert_eq!(color_to_fg_ansi(&Color::Cyan), "36");
        assert_eq!(color_to_fg_ansi(&Color::DarkCyan), "96");
        assert_eq!(color_to_fg_ansi(&Color::White), "37");
        assert_eq!(color_to_fg_ansi(&Color::Grey), "97");
        assert_eq!(color_to_fg_ansi(&Color::AnsiValue(196)), "38;5;196");
        assert_eq!(
            color_to_fg_ansi(&Color::Rgb {
                r: 10,
                g: 20,
                b: 30
            }),
            "38;2;10;20;30"
        );
    }

    #[test]
    fn test_color_to_bg_ansi_direct_all() {
        assert_eq!(color_to_bg_ansi(&Color::Reset), "49");
        assert_eq!(color_to_bg_ansi(&Color::Black), "40");
        assert_eq!(color_to_bg_ansi(&Color::DarkGrey), "100");
        assert_eq!(color_to_bg_ansi(&Color::Red), "41");
        assert_eq!(color_to_bg_ansi(&Color::DarkRed), "101");
        assert_eq!(color_to_bg_ansi(&Color::Green), "42");
        assert_eq!(color_to_bg_ansi(&Color::DarkGreen), "102");
        assert_eq!(color_to_bg_ansi(&Color::Yellow), "43");
        assert_eq!(color_to_bg_ansi(&Color::DarkYellow), "103");
        assert_eq!(color_to_bg_ansi(&Color::Blue), "44");
        assert_eq!(color_to_bg_ansi(&Color::DarkBlue), "104");
        assert_eq!(color_to_bg_ansi(&Color::Magenta), "45");
        assert_eq!(color_to_bg_ansi(&Color::DarkMagenta), "105");
        assert_eq!(color_to_bg_ansi(&Color::Cyan), "46");
        assert_eq!(color_to_bg_ansi(&Color::DarkCyan), "106");
        assert_eq!(color_to_bg_ansi(&Color::White), "47");
        assert_eq!(color_to_bg_ansi(&Color::Grey), "107");
        assert_eq!(color_to_bg_ansi(&Color::AnsiValue(52)), "48;5;52");
        assert_eq!(
            color_to_bg_ansi(&Color::Rgb {
                r: 10,
                g: 20,
                b: 30
            }),
            "48;2;10;20;30"
        );
    }

    #[test]
    fn test_color_to_underline_ansi_direct_all() {
        assert_eq!(color_to_underline_ansi(&Color::Reset), "59");
        assert_eq!(color_to_underline_ansi(&Color::AnsiValue(42)), "58;5;42");
        assert_eq!(color_to_underline_ansi(&Color::Rgb { r: 1, g: 2, b: 3 }), "58;2;1;2;3");
        assert_eq!(color_to_underline_ansi(&Color::Black), "58;5;0");
        assert_eq!(color_to_underline_ansi(&Color::DarkRed), "58;5;1");
        assert_eq!(color_to_underline_ansi(&Color::DarkGreen), "58;5;2");
        assert_eq!(color_to_underline_ansi(&Color::DarkYellow), "58;5;3");
        assert_eq!(color_to_underline_ansi(&Color::DarkBlue), "58;5;4");
        assert_eq!(color_to_underline_ansi(&Color::DarkMagenta), "58;5;5");
        assert_eq!(color_to_underline_ansi(&Color::DarkCyan), "58;5;6");
        assert_eq!(color_to_underline_ansi(&Color::Grey), "58;5;7");
        assert_eq!(color_to_underline_ansi(&Color::DarkGrey), "58;5;8");
        assert_eq!(color_to_underline_ansi(&Color::Red), "58;5;9");
        assert_eq!(color_to_underline_ansi(&Color::Green), "58;5;10");
        assert_eq!(color_to_underline_ansi(&Color::Yellow), "58;5;11");
        assert_eq!(color_to_underline_ansi(&Color::Blue), "58;5;12");
        assert_eq!(color_to_underline_ansi(&Color::Magenta), "58;5;13");
        assert_eq!(color_to_underline_ansi(&Color::Cyan), "58;5;14");
        assert_eq!(color_to_underline_ansi(&Color::White), "58;5;15");
    }

    #[test]
    fn test_ansi_index_to_color_direct_all() {
        assert_eq!(ansi_index_to_color(0), Color::Black);
        assert_eq!(ansi_index_to_color(1), Color::DarkRed);
        assert_eq!(ansi_index_to_color(2), Color::DarkGreen);
        assert_eq!(ansi_index_to_color(3), Color::DarkYellow);
        assert_eq!(ansi_index_to_color(4), Color::DarkBlue);
        assert_eq!(ansi_index_to_color(5), Color::DarkMagenta);
        assert_eq!(ansi_index_to_color(6), Color::DarkCyan);
        assert_eq!(ansi_index_to_color(7), Color::Grey);
        assert_eq!(ansi_index_to_color(8), Color::DarkGrey);
        assert_eq!(ansi_index_to_color(9), Color::Red);
        assert_eq!(ansi_index_to_color(10), Color::Green);
        assert_eq!(ansi_index_to_color(11), Color::Yellow);
        assert_eq!(ansi_index_to_color(12), Color::Blue);
        assert_eq!(ansi_index_to_color(13), Color::Magenta);
        assert_eq!(ansi_index_to_color(14), Color::Cyan);
        assert_eq!(ansi_index_to_color(15), Color::White);
    }

    #[test]
    fn test_ansi256_to_ansi16_grayscale_direct() {
        let dark = ansi256_to_ansi16(232);
        assert_eq!(dark, Color::Black);
        let light = ansi256_to_ansi16(255);
        assert_eq!(light, Color::White);
        let mid = ansi256_to_ansi16(244);
        assert!(
            matches!(mid, Color::Grey | Color::DarkGrey | Color::White),
            "mid-grayscale {mid:?} should be a grey/white variant"
        );
    }

    #[test]
    fn test_ansi256_to_ansi16_color_cube_direct() {
        let red = ansi256_to_ansi16(196);
        assert_eq!(red, Color::Red);
        let green = ansi256_to_ansi16(46);
        assert_eq!(green, Color::Green);
        let blue = ansi256_to_ansi16(21);
        assert_eq!(blue, Color::Blue);
    }

    #[test]
    fn test_rgb_to_nearest_ansi16_all_exact() {
        for (idx, &(r, g, b)) in ANSI16_RGB.iter().enumerate() {
            let result = rgb_to_nearest_ansi16(r, g, b);
            #[allow(clippy::cast_possible_truncation)]
            let expected = ansi_index_to_color(idx as u8);
            assert_eq!(
                result, expected,
                "RGB ({r},{g},{b}) at index {idx} should map to {expected:?}, got {result:?}"
            );
        }
    }

    #[test]
    fn test_attributes_from_str_infallible() {
        let result: Result<Attributes, std::convert::Infallible> = "bold".parse();
        assert!(result.is_ok());
        let attrs = result.unwrap();
        assert!(attrs.contains(Attributes::BOLD));
    }

    #[test]
    fn test_style_struct_fields_default() {
        let style = Style::default();
        assert_eq!(style.attributes, Attributes::default());
        assert!(style.underline_color.is_none());
    }
}
