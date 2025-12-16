//! Theme system for semantic color definitions

use crate::highlight::Style;
use reovim_sys::style::Color;

/// Semantic style elements for the editor UI
#[derive(Debug, Clone)]
pub struct Theme {
    /// Default text foreground and background
    pub default: Style,
    /// Line numbers in the gutter
    pub line_number: Style,
    /// Current line number (cursor line)
    pub current_line_number: Style,
    /// Visual mode selection
    pub visual_selection: Style,
    /// Status line background
    pub status_line: Style,
    /// Mode indicator in status line (NORMAL, INSERT, etc.)
    pub status_line_mode: StatusLineModeStyles,
    /// Command line (: commands)
    pub command_line: Style,
    /// Cursor line background highlight
    pub cursor_line: Style,
    /// Completion popup - normal item
    pub popup_normal: Style,
    /// Completion popup - selected item
    pub popup_selected: Style,
}

/// Mode-specific styles for the status line
#[derive(Debug, Clone)]
pub struct StatusLineModeStyles {
    pub normal: Style,
    pub insert: Style,
    pub visual: Style,
    pub command: Style,
}

impl Default for StatusLineModeStyles {
    fn default() -> Self {
        Self {
            normal: Style::new()
                .fg(Color::Black)
                .bg(Color::Green)
                .bold(),
            insert: Style::new()
                .fg(Color::Black)
                .bg(Color::Blue)
                .bold(),
            visual: Style::new()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .bold(),
            command: Style::new()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .bold(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Create the default dark theme
    #[must_use]
    pub fn dark() -> Self {
        Self {
            default: Style::new(),

            // Line numbers: dim gray
            line_number: Style::new().fg(Color::DarkGrey),

            // Current line number: bright yellow, bold
            current_line_number: Style::new()
                .fg(Color::Yellow)
                .bold(),

            // Visual selection: gray background
            visual_selection: Style::new().bg(Color::AnsiValue(240)),

            // Status line: inverted (white on dark)
            status_line: Style::new()
                .fg(Color::White)
                .bg(Color::AnsiValue(236)),

            // Mode-specific status line styles
            status_line_mode: StatusLineModeStyles::default(),

            // Command line: default colors
            command_line: Style::new(),

            // Cursor line: subtle highlight
            cursor_line: Style::new().bg(Color::AnsiValue(235)),

            // Completion popup: dark background
            popup_normal: Style::new()
                .fg(Color::White)
                .bg(Color::AnsiValue(238)),
            popup_selected: Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan),
        }
    }

    /// Create a light theme
    #[must_use]
    pub fn light() -> Self {
        Self {
            default: Style::new()
                .fg(Color::Black)
                .bg(Color::White),

            line_number: Style::new().fg(Color::Grey),

            current_line_number: Style::new()
                .fg(Color::DarkBlue)
                .bold(),

            visual_selection: Style::new().bg(Color::AnsiValue(153)), // Light blue

            status_line: Style::new()
                .fg(Color::Black)
                .bg(Color::AnsiValue(250)),

            status_line_mode: StatusLineModeStyles {
                normal: Style::new()
                    .fg(Color::White)
                    .bg(Color::DarkGreen)
                    .bold(),
                insert: Style::new()
                    .fg(Color::White)
                    .bg(Color::DarkBlue)
                    .bold(),
                visual: Style::new()
                    .fg(Color::White)
                    .bg(Color::DarkMagenta)
                    .bold(),
                command: Style::new()
                    .fg(Color::Black)
                    .bg(Color::DarkYellow)
                    .bold(),
            },

            command_line: Style::new()
                .fg(Color::Black)
                .bg(Color::White),

            cursor_line: Style::new().bg(Color::AnsiValue(254)),

            // Completion popup: light background
            popup_normal: Style::new()
                .fg(Color::Black)
                .bg(Color::AnsiValue(252)),
            popup_selected: Style::new()
                .fg(Color::White)
                .bg(Color::DarkBlue),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        // Line number should have foreground color
        assert!(theme.line_number.fg.is_some());
        // Current line number should be bold
        assert!(theme.current_line_number.attributes.contains(crate::highlight::Attributes::BOLD));
    }

    #[test]
    fn test_dark_theme() {
        let theme = Theme::dark();
        // Visual selection should have background
        assert!(theme.visual_selection.bg.is_some());
    }

    #[test]
    fn test_light_theme() {
        let theme = Theme::light();
        // Default should have both fg and bg
        assert!(theme.default.fg.is_some());
        assert!(theme.default.bg.is_some());
    }

    #[test]
    fn test_status_line_modes() {
        let modes = StatusLineModeStyles::default();
        // All modes should have background color
        assert!(modes.normal.bg.is_some());
        assert!(modes.insert.bg.is_some());
        assert!(modes.visual.bg.is_some());
        assert!(modes.command.bg.is_some());
    }
}
