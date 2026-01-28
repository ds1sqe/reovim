//! Statusline theme configuration.
//!
//! Provides mode-specific colors and section gradients for the statusline.
//!
//! # Lualine-Style Theme
//!
//! The default theme follows lualine conventions:
//! - Mode indicator (Section A) changes color based on current mode
//! - Sections have a gradient from dark (A/Z) to light (C/X)
//! - Powerline separators use color transitions
//!
//! # Mode Colors
//!
//! ```text
//! NORMAL   → Blue background
//! INSERT   → Green background
//! VISUAL   → Magenta background
//! REPLACE  → Red background
//! COMMAND  → Yellow background
//! SELECT   → Cyan background
//! TERMINAL → Green background
//! OPERATOR → Magenta background
//! ```

use reovim_driver_display::{Color, Style};

/// Mode-specific colors for section A (mode indicator).
///
/// Each mode has a distinct color to provide visual feedback.
#[derive(Debug, Clone)]
pub struct ModeColors {
    /// Normal mode color (default: blue).
    pub normal: Style,
    /// Insert mode color (default: green).
    pub insert: Style,
    /// Visual mode color (default: magenta).
    pub visual: Style,
    /// Visual line mode color (default: magenta).
    pub visual_line: Style,
    /// Visual block mode color (default: magenta).
    pub visual_block: Style,
    /// Replace mode color (default: red).
    pub replace: Style,
    /// Command mode color (default: yellow).
    pub command: Style,
    /// Select mode color (default: cyan).
    pub select: Style,
    /// Terminal mode color (default: green).
    pub terminal: Style,
    /// Operator-pending mode color (default: magenta).
    pub operator: Style,
}

impl Default for ModeColors {
    fn default() -> Self {
        Self {
            normal: Style::new().bg(Color::Blue).fg(Color::Black),
            insert: Style::new().bg(Color::Green).fg(Color::Black),
            visual: Style::new().bg(Color::Magenta).fg(Color::Black),
            visual_line: Style::new().bg(Color::Magenta).fg(Color::Black),
            visual_block: Style::new().bg(Color::Magenta).fg(Color::Black),
            replace: Style::new().bg(Color::Red).fg(Color::White),
            command: Style::new().bg(Color::Yellow).fg(Color::Black),
            select: Style::new().bg(Color::Cyan).fg(Color::Black),
            terminal: Style::new().bg(Color::Green).fg(Color::Black),
            operator: Style::new().bg(Color::Magenta).fg(Color::Black),
        }
    }
}

impl ModeColors {
    /// Get the style for a given mode name.
    #[must_use]
    pub fn style_for_mode(&self, mode: &str) -> Style {
        match mode.to_uppercase().as_str() {
            "INSERT" => self.insert.clone(),
            "VISUAL" => self.visual.clone(),
            "VISUAL_LINE" | "V-LINE" => self.visual_line.clone(),
            "VISUAL_BLOCK" | "V-BLOCK" => self.visual_block.clone(),
            "REPLACE" => self.replace.clone(),
            "COMMAND" => self.command.clone(),
            "SELECT" => self.select.clone(),
            "TERMINAL" => self.terminal.clone(),
            "OPERATOR" | "OPERATOR_PENDING" => self.operator.clone(),
            // Default to normal for unknown modes
            _ => self.normal.clone(),
        }
    }

    /// Get the background color for a given mode.
    #[must_use]
    pub fn bg_for_mode(&self, mode: &str) -> Option<Color> {
        self.style_for_mode(mode).bg
    }
}

/// Section colors for lualine-style gradient.
///
/// Sections have a gradient from dark edges (A/Z) to light center (C/X):
/// ```text
/// │ A (dark) │ B (medium) │ C (light) │ ... │ X (light) │ Y (medium) │ Z (dark) │
/// ```
#[derive(Debug, Clone)]
pub struct SectionColors {
    /// Section A style (darkest, mode indicator).
    pub a: Style,
    /// Section B style (medium dark).
    pub b: Style,
    /// Section C style (light, content area).
    pub c: Style,
    /// Section X style (light, content area).
    pub x: Style,
    /// Section Y style (medium dark).
    pub y: Style,
    /// Section Z style (darkest).
    pub z: Style,
}

impl Default for SectionColors {
    fn default() -> Self {
        // Default gradient using 256-color palette
        Self {
            a: Style::new().bg(Color::Blue).fg(Color::Black),
            b: Style::new().bg(Color::AnsiValue(240)).fg(Color::White), // Dark gray
            c: Style::new()
                .bg(Color::AnsiValue(238))
                .fg(Color::AnsiValue(250)), // Medium gray
            x: Style::new()
                .bg(Color::AnsiValue(238))
                .fg(Color::AnsiValue(250)), // Medium gray
            y: Style::new().bg(Color::AnsiValue(240)).fg(Color::White), // Dark gray
            z: Style::new().bg(Color::Blue).fg(Color::Black),
        }
    }
}

impl SectionColors {
    /// Create section colors from mode colors.
    ///
    /// Uses the mode's background color for sections A and Z,
    /// and generates a gradient for the middle sections.
    #[must_use]
    pub fn from_mode(mode_style: &Style) -> Self {
        let background = mode_style.bg.unwrap_or(Color::Blue);
        let foreground = mode_style.fg.unwrap_or(Color::Black);

        Self {
            a: mode_style.clone(),
            b: Style::new().bg(Color::AnsiValue(240)).fg(Color::White),
            c: Style::new()
                .bg(Color::AnsiValue(238))
                .fg(Color::AnsiValue(250)),
            x: Style::new()
                .bg(Color::AnsiValue(238))
                .fg(Color::AnsiValue(250)),
            y: Style::new().bg(Color::AnsiValue(240)).fg(Color::White),
            z: Style::new().bg(background).fg(foreground),
        }
    }

    /// Get the style for a section by ID.
    #[must_use]
    pub fn style_for_section(&self, section: reovim_driver_display::SectionId) -> Style {
        use reovim_driver_display::SectionId;
        match section {
            SectionId::A => self.a.clone(),
            SectionId::B => self.b.clone(),
            SectionId::C => self.c.clone(),
            SectionId::X => self.x.clone(),
            SectionId::Y => self.y.clone(),
            SectionId::Z => self.z.clone(),
        }
    }
}

/// Complete statusline theme configuration.
#[derive(Debug, Clone, Default)]
pub struct StatuslineTheme {
    /// Mode-specific colors for section A.
    pub mode_colors: ModeColors,
    /// Default section colors (used when mode colors are not applied).
    pub section_colors: SectionColors,
}

impl StatuslineTheme {
    /// Create a new theme with default colors.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get section colors based on the current mode.
    ///
    /// Returns section colors with A and Z styled according to the mode.
    #[must_use]
    pub fn section_colors_for_mode(&self, mode: &str) -> SectionColors {
        let mode_style = self.mode_colors.style_for_mode(mode);
        SectionColors::from_mode(&mode_style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_colors_default() {
        let colors = ModeColors::default();
        assert_eq!(colors.normal.bg, Some(Color::Blue));
        assert_eq!(colors.insert.bg, Some(Color::Green));
        assert_eq!(colors.visual.bg, Some(Color::Magenta));
    }

    #[test]
    fn test_mode_colors_style_for_mode() {
        let colors = ModeColors::default();

        let normal = colors.style_for_mode("NORMAL");
        assert_eq!(normal.bg, Some(Color::Blue));

        let insert = colors.style_for_mode("INSERT");
        assert_eq!(insert.bg, Some(Color::Green));

        let visual = colors.style_for_mode("VISUAL");
        assert_eq!(visual.bg, Some(Color::Magenta));

        // Test case insensitivity
        let insert_lower = colors.style_for_mode("insert");
        assert_eq!(insert_lower.bg, Some(Color::Green));
    }

    #[test]
    fn test_section_colors_default() {
        let colors = SectionColors::default();
        assert_eq!(colors.a.bg, Some(Color::Blue));
        assert_eq!(colors.z.bg, Some(Color::Blue));
    }

    #[test]
    fn test_section_colors_from_mode() {
        let mode_style = Style::new().bg(Color::Green).fg(Color::Black);
        let colors = SectionColors::from_mode(&mode_style);

        // A and Z should have the mode color
        assert_eq!(colors.a.bg, Some(Color::Green));
        assert_eq!(colors.z.bg, Some(Color::Green));

        // B and Y should be the gradient color
        assert_eq!(colors.b.bg, Some(Color::AnsiValue(240)));
        assert_eq!(colors.y.bg, Some(Color::AnsiValue(240)));
    }

    #[test]
    fn test_theme_section_colors_for_mode() {
        let theme = StatuslineTheme::new();

        let normal_colors = theme.section_colors_for_mode("NORMAL");
        assert_eq!(normal_colors.a.bg, Some(Color::Blue));

        let insert_colors = theme.section_colors_for_mode("INSERT");
        assert_eq!(insert_colors.a.bg, Some(Color::Green));
    }
}
