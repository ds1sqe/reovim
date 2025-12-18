//! Terminal color capability detection and conversion

use std::sync::LazyLock;

use reovim_sys::style::Color;

/// Terminal color capability levels
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ColorMode {
    /// 16 ANSI colors (basic terminal)
    Ansi16,
    /// 256 color palette
    Color256,
    /// 24-bit true color (RGB)
    #[default]
    TrueColor,
}

impl ColorMode {
    /// Detect terminal color capability from environment variables
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

    /// Parse from string for :set command
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

/// Pre-computed lookup table: 256-color palette to nearest ANSI 16
static ANSI256_TO_16: LazyLock<[Color; 256]> = LazyLock::new(build_256_to_16_table);

/// The 16 ANSI colors as (R, G, B) for distance calculation
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

/// Build the 256 -> 16 color lookup table
fn build_256_to_16_table() -> [Color; 256] {
    let mut table = [Color::Black; 256];

    // Standard ANSI colors (0-15) map to themselves
    for i in 0..16u8 {
        table[i as usize] = ansi_index_to_color(i);
    }

    // 6x6x6 color cube (16-231)
    for i in 16..232u8 {
        let rgb = ansi256_to_rgb(i);
        table[i as usize] = rgb_to_nearest_ansi16(rgb.0, rgb.1, rgb.2);
    }

    // Grayscale ramp (232-255)
    for i in 232..=255u8 {
        let gray = 8 + (i - 232) * 10;
        table[i as usize] = rgb_to_nearest_ansi16(gray, gray, gray);
    }

    table
}

/// Convert ANSI 256 index to RGB
fn ansi256_to_rgb(index: u8) -> (u8, u8, u8) {
    if index < 16 {
        // Standard colors - use ANSI16_RGB values
        ANSI16_RGB[index as usize]
    } else if index < 232 {
        // 6x6x6 color cube
        let idx = index - 16;
        let r = (idx / 36) % 6;
        let g = (idx / 6) % 6;
        let b = idx % 6;
        let to_rgb = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        (to_rgb(r), to_rgb(g), to_rgb(b))
    } else {
        // Grayscale (232-255)
        let gray = 8 + (index - 232) * 10;
        (gray, gray, gray)
    }
}

/// Convert index 0-15 to Color enum
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

/// Find nearest ANSI 16 color using squared Euclidean distance
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
            // idx is always 0-15, safe to cast to u8
            best_idx = idx as u8;
        }
    }

    ansi_index_to_color(best_idx)
}

/// Convert RGB to nearest 256-color palette entry
#[must_use]
pub fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    // Check if it's a grayscale
    if r == g && g == b {
        if r < 8 {
            return 16; // Near black in cube
        }
        if r > 248 {
            return 231; // Near white in cube
        }
        return 232 + ((r - 8) / 10).min(23);
    }

    // Map to 6x6x6 color cube
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

/// Downgrade a color to the specified mode
#[must_use]
pub fn downgrade_color(color: Color, mode: ColorMode) -> Color {
    match (color, mode) {
        // 256-color mode: convert RGB to 256
        (Color::Rgb { r, g, b }, ColorMode::Color256) => Color::AnsiValue(rgb_to_ansi256(r, g, b)),

        // ANSI 16 mode: convert everything to basic colors
        (Color::Rgb { r, g, b }, ColorMode::Ansi16) => rgb_to_nearest_ansi16(r, g, b),
        (Color::AnsiValue(n), ColorMode::Ansi16) => ANSI256_TO_16[n as usize],

        // All other cases pass through unchanged:
        // - TrueColor mode: no conversion needed
        // - Color256 mode with non-RGB: AnsiValue and basic colors pass through
        // - Ansi16 mode with basic colors: pass through
        _ => color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colormode_parse() {
        assert_eq!(ColorMode::parse("ansi"), Some(ColorMode::Ansi16));
        assert_eq!(ColorMode::parse("ANSI16"), Some(ColorMode::Ansi16));
        assert_eq!(ColorMode::parse("16"), Some(ColorMode::Ansi16));
        assert_eq!(ColorMode::parse("256"), Some(ColorMode::Color256));
        assert_eq!(ColorMode::parse("256color"), Some(ColorMode::Color256));
        assert_eq!(ColorMode::parse("truecolor"), Some(ColorMode::TrueColor));
        assert_eq!(ColorMode::parse("rgb"), Some(ColorMode::TrueColor));
        assert_eq!(ColorMode::parse("24bit"), Some(ColorMode::TrueColor));
        assert_eq!(ColorMode::parse("invalid"), None);
    }

    #[test]
    fn test_rgb_to_ansi256_grayscale() {
        // Pure black -> near black in cube
        assert_eq!(rgb_to_ansi256(0, 0, 0), 16);
        // Pure white -> near white in cube
        assert_eq!(rgb_to_ansi256(255, 255, 255), 231);
        // Mid gray -> grayscale ramp (232-255)
        let gray = rgb_to_ansi256(128, 128, 128);
        assert!(gray >= 232);
    }

    #[test]
    fn test_rgb_to_ansi256_colors() {
        // Pure red should map to red area of cube
        let red = rgb_to_ansi256(255, 0, 0);
        assert!((16..232).contains(&red));
        // Pure green
        let green = rgb_to_ansi256(0, 255, 0);
        assert!((16..232).contains(&green));
        // Pure blue
        let blue = rgb_to_ansi256(0, 0, 255);
        assert!((16..232).contains(&blue));
    }

    #[test]
    fn test_downgrade_color_truecolor() {
        // TrueColor mode passes everything through
        let rgb = Color::Rgb {
            r: 255,
            g: 128,
            b: 64,
        };
        assert_eq!(downgrade_color(rgb, ColorMode::TrueColor), rgb);

        let ansi = Color::AnsiValue(196);
        assert_eq!(downgrade_color(ansi, ColorMode::TrueColor), ansi);

        assert_eq!(downgrade_color(Color::Red, ColorMode::TrueColor), Color::Red);
    }

    #[test]
    fn test_downgrade_color_256() {
        // RGB converts to AnsiValue
        let rgb = Color::Rgb { r: 255, g: 0, b: 0 };
        let converted = downgrade_color(rgb, ColorMode::Color256);
        assert!(matches!(converted, Color::AnsiValue(_)));

        // AnsiValue passes through
        let ansi = Color::AnsiValue(196);
        assert_eq!(downgrade_color(ansi, ColorMode::Color256), ansi);

        // Basic colors pass through
        assert_eq!(downgrade_color(Color::Red, ColorMode::Color256), Color::Red);
    }

    #[test]
    fn test_downgrade_color_ansi16() {
        // RGB converts to basic color
        let rgb = Color::Rgb { r: 255, g: 0, b: 0 };
        let converted = downgrade_color(rgb, ColorMode::Ansi16);
        // Should be Red or DarkRed
        assert!(matches!(converted, Color::Red | Color::DarkRed));

        // AnsiValue converts to basic color
        let ansi = Color::AnsiValue(196); // Red in 256 palette
        let converted = downgrade_color(ansi, ColorMode::Ansi16);
        assert!(!matches!(converted, Color::AnsiValue(_) | Color::Rgb { .. }));

        // Basic colors pass through
        assert_eq!(downgrade_color(Color::Blue, ColorMode::Ansi16), Color::Blue);
    }

    #[test]
    fn test_ansi256_to_rgb_standard() {
        // Standard colors match ANSI16_RGB
        for i in 0..16u8 {
            assert_eq!(ansi256_to_rgb(i), ANSI16_RGB[i as usize]);
        }
    }

    #[test]
    fn test_ansi256_to_rgb_cube() {
        // Color cube corners
        assert_eq!(ansi256_to_rgb(16), (0, 0, 0)); // Black corner
        assert_eq!(ansi256_to_rgb(21), (0, 0, 255)); // Blue corner (index 16 + 5)
        assert_eq!(ansi256_to_rgb(196), (255, 0, 0)); // Red corner (index 16 + 36*5)
        assert_eq!(ansi256_to_rgb(231), (255, 255, 255)); // White corner
    }

    #[test]
    fn test_ansi256_to_rgb_grayscale() {
        // Grayscale ramp
        assert_eq!(ansi256_to_rgb(232), (8, 8, 8)); // Darkest gray
        assert_eq!(ansi256_to_rgb(255), (238, 238, 238)); // Lightest gray
    }

    #[test]
    fn test_lookup_table_initialized() {
        // Access the lookup table to ensure it initializes without panic
        let _ = ANSI256_TO_16[0];
        let _ = ANSI256_TO_16[255];
    }
}
