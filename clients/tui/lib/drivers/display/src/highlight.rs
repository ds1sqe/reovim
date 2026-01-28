//! Style types for display rendering.
//!
//! These types were migrated from lib/core/src/highlight/ to break the
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
}
