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
    /// Telescope - border style
    pub telescope_border: Style,
    /// Telescope - normal item
    pub telescope_normal: Style,
    /// Telescope - selected item
    pub telescope_selected: Style,
    /// Telescope - preview pane
    pub telescope_preview: Style,
    /// Telescope - preview highlighted line
    pub telescope_preview_highlight: Style,
    /// Telescope - prompt text (e.g., "> ")
    pub telescope_prompt: Style,
    /// Telescope - input text
    pub telescope_input: Style,
    /// Which-key popup panel styles
    pub which_key: WhichKeyStyles,
    /// Leap motion labels (overlay on matches)
    pub leap_label: Style,
    /// Fold marker (collapsed fold indicator)
    pub fold_marker: Style,
    /// Indent guide (vertical line at indent levels)
    pub indent_guide: Style,
    /// Active indent guide (at cursor's indent level)
    pub indent_guide_active: Style,
    /// Active tab in tab line
    pub tab_active: Style,
    /// Inactive tab in tab line
    pub tab_inactive: Style,
    /// Tab line fill (empty space)
    pub tab_fill: Style,
    /// Window separator (between split windows)
    pub window_separator: Style,
}

/// Mode-specific styles for the status line
#[derive(Debug, Clone)]
pub struct StatusLineModeStyles {
    pub normal: Style,
    pub insert: Style,
    pub visual: Style,
    pub command: Style,
    pub explorer: Style,
}

/// Styles for the which-key popup panel
#[derive(Debug, Clone)]
pub struct WhichKeyStyles {
    /// Panel background
    pub background: Style,
    /// Key bindings (the key part)
    pub key: Style,
    /// Description text
    pub description: Style,
    /// Prefix indicator (+)
    pub prefix: Style,
    /// Border/separator style
    pub border: Style,
}

impl Default for WhichKeyStyles {
    fn default() -> Self {
        // OneDark-inspired colors
        let bg_medium = Color::Rgb { r: 40, g: 44, b: 52 }; // #282c34
        let fg = Color::Rgb { r: 171, g: 178, b: 191 }; // #abb2bf
        let yellow = Color::Rgb { r: 229, g: 192, b: 123 }; // #e5c07b
        let cyan = Color::Rgb { r: 86, g: 182, b: 194 }; // #56b6c2
        let fg_dark = Color::Rgb { r: 92, g: 99, b: 112 }; // #5c6370

        Self {
            background: Style::new().bg(bg_medium),
            key: Style::new().fg(yellow).bg(bg_medium).bold(),
            description: Style::new().fg(fg).bg(bg_medium),
            prefix: Style::new().fg(cyan).bg(bg_medium).bold(),
            border: Style::new().fg(fg_dark).bg(bg_medium),
        }
    }
}

impl Default for StatusLineModeStyles {
    fn default() -> Self {
        // OneDark-inspired colors
        let bg_dark = Color::Rgb { r: 33, g: 37, b: 43 }; // #21252b
        let green = Color::Rgb { r: 152, g: 195, b: 121 }; // #98c379
        let blue = Color::Rgb { r: 97, g: 175, b: 239 }; // #61afef
        let magenta = Color::Rgb { r: 198, g: 120, b: 221 }; // #c678dd
        let yellow = Color::Rgb { r: 229, g: 192, b: 123 }; // #e5c07b
        let cyan = Color::Rgb { r: 86, g: 182, b: 194 }; // #56b6c2

        Self {
            normal: Style::new().fg(bg_dark).bg(green).bold(),
            insert: Style::new().fg(bg_dark).bg(blue).bold(),
            visual: Style::new().fg(bg_dark).bg(magenta).bold(),
            command: Style::new().fg(bg_dark).bg(yellow).bold(),
            explorer: Style::new().fg(bg_dark).bg(cyan).bold(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Create the default dark theme (OneDark-inspired)
    #[must_use]
    pub fn dark() -> Self {
        // OneDark palette
        let bg_light = Color::Rgb { r: 50, g: 56, b: 66 }; // #323842
        let bg_medium = Color::Rgb { r: 40, g: 44, b: 52 }; // #282c34
        let fg = Color::Rgb { r: 171, g: 178, b: 191 }; // #abb2bf
        let fg_dark = Color::Rgb { r: 92, g: 99, b: 112 }; // #5c6370
        let yellow = Color::Rgb { r: 229, g: 192, b: 123 }; // #e5c07b
        let blue = Color::Rgb { r: 97, g: 175, b: 239 }; // #61afef
        let bg_dark = Color::Rgb { r: 33, g: 37, b: 43 }; // #21252b

        Self {
            default: Style::new(),

            // Line numbers: muted
            line_number: Style::new().fg(fg_dark),

            // Current line number: yellow, bold
            current_line_number: Style::new().fg(yellow).bold(),

            // Visual selection: subtle highlight
            visual_selection: Style::new().bg(bg_light),

            // Status line: subtle contrast
            status_line: Style::new().fg(fg).bg(bg_light),

            // Mode-specific status line styles
            status_line_mode: StatusLineModeStyles::default(),

            // Command line: default colors
            command_line: Style::new(),

            // Cursor line: subtle highlight
            cursor_line: Style::new().bg(bg_light),

            // Completion popup
            popup_normal: Style::new().fg(fg).bg(bg_medium),
            popup_selected: Style::new().fg(bg_dark).bg(blue),

            // Telescope: floating panel style
            telescope_border: Style::new().fg(blue),
            telescope_normal: Style::new().fg(fg).bg(bg_medium),
            telescope_selected: Style::new().fg(bg_dark).bg(blue),
            telescope_preview: Style::new().fg(fg_dark).bg(bg_medium),
            telescope_preview_highlight: Style::new().fg(yellow).bg(bg_light),
            telescope_prompt: Style::new().fg(blue).bold(),
            telescope_input: Style::new().fg(fg),

            // Which-key panel
            which_key: WhichKeyStyles::default(),

            // Leap labels: bright contrast for visibility
            leap_label: Style::new().fg(bg_dark).bg(yellow).bold(),

            // Fold marker: muted yellow/orange for collapsed fold indicators
            fold_marker: Style::new().fg(fg_dark).italic(),

            // Indent guides: subtle vertical lines
            indent_guide: Style::new().fg(fg_dark),
            indent_guide_active: Style::new().fg(fg),

            // Tab line styles
            tab_active: Style::new().fg(fg).bg(bg_medium).bold(),
            tab_inactive: Style::new().fg(fg_dark).bg(bg_dark),
            tab_fill: Style::new().bg(bg_dark),

            // Window separator
            window_separator: Style::new().fg(fg_dark),
        }
    }

    /// Create a light theme
    #[must_use]
    pub fn light() -> Self {
        let fg_light = Color::Rgb { r: 56, g: 58, b: 66 }; // #383a42
        let bg_light = Color::Rgb { r: 250, g: 250, b: 250 }; // #fafafa
        let bg_medium = Color::Rgb { r: 233, g: 233, b: 233 }; // #e9e9e9

        Self {
            default: Style::new().fg(fg_light).bg(bg_light),

            line_number: Style::new().fg(Color::Grey),

            current_line_number: Style::new().fg(Color::DarkBlue).bold(),

            visual_selection: Style::new().bg(Color::AnsiValue(153)), // Light blue

            status_line: Style::new().fg(fg_light).bg(bg_medium),

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
                explorer: Style::new()
                    .fg(Color::White)
                    .bg(Color::DarkCyan)
                    .bold(),
            },

            command_line: Style::new().fg(fg_light).bg(bg_light),

            cursor_line: Style::new().bg(Color::AnsiValue(254)),

            // Completion popup: light background
            popup_normal: Style::new().fg(fg_light).bg(Color::AnsiValue(252)),
            popup_selected: Style::new().fg(Color::White).bg(Color::DarkBlue),

            // Telescope: floating panel style (light)
            telescope_border: Style::new().fg(Color::DarkBlue),
            telescope_normal: Style::new().fg(fg_light).bg(Color::AnsiValue(254)),
            telescope_selected: Style::new().fg(Color::White).bg(Color::DarkBlue),
            telescope_preview: Style::new().fg(Color::DarkGrey).bg(Color::AnsiValue(255)),
            telescope_preview_highlight: Style::new().fg(Color::DarkBlue).bg(Color::AnsiValue(250)),
            telescope_prompt: Style::new().fg(Color::DarkBlue).bold(),
            telescope_input: Style::new().fg(fg_light),

            // Which-key panel: light version
            which_key: WhichKeyStyles {
                background: Style::new().bg(bg_medium),
                key: Style::new().fg(Color::DarkBlue).bg(bg_medium).bold(),
                description: Style::new().fg(fg_light).bg(bg_medium),
                prefix: Style::new().fg(Color::DarkCyan).bg(bg_medium).bold(),
                border: Style::new().fg(Color::Grey).bg(bg_medium),
            },

            // Leap labels: bright contrast for visibility
            leap_label: Style::new().fg(Color::White).bg(Color::DarkYellow).bold(),

            // Fold marker: light version
            fold_marker: Style::new().fg(Color::Grey).italic(),

            // Indent guides: light version
            indent_guide: Style::new().fg(Color::AnsiValue(250)),
            indent_guide_active: Style::new().fg(Color::Grey),

            // Tab line styles
            tab_active: Style::new().fg(fg_light).bg(bg_light).bold(),
            tab_inactive: Style::new().fg(Color::Grey).bg(bg_medium),
            tab_fill: Style::new().bg(bg_medium),

            // Window separator
            window_separator: Style::new().fg(Color::Grey),
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
        assert!(modes.explorer.bg.is_some());
    }
}
