//! Markdown renderer configuration

use reovim_core::{highlight::Style, sys::style::Color};

/// Configuration for heading rendering
#[derive(Debug, Clone)]
pub struct HeadingConfig {
    /// Icons for each heading level (H1-H6)
    pub icons: [&'static str; 6],
    /// Text styles for each heading level
    pub styles: [Style; 6],
    /// Background styles for each heading level (None = no background)
    pub backgrounds: [Option<Style>; 6],
}

impl Default for HeadingConfig {
    fn default() -> Self {
        Self {
            // Nerd Font icons for headings
            icons: ["󰉫 ", "󰉬 ", "󰉭 ", "󰉮 ", "󰉯 ", "󰉰 "],
            styles: [
                Style::new()
                    .fg(Color::Rgb {
                        r: 255,
                        g: 158,
                        b: 100,
                    })
                    .bold(), // H1: Orange
                Style::new()
                    .fg(Color::Rgb {
                        r: 158,
                        g: 206,
                        b: 106,
                    })
                    .bold(), // H2: Green
                Style::new()
                    .fg(Color::Rgb {
                        r: 122,
                        g: 162,
                        b: 247,
                    })
                    .bold(), // H3: Blue
                Style::new()
                    .fg(Color::Rgb {
                        r: 224,
                        g: 175,
                        b: 104,
                    })
                    .bold(), // H4: Yellow
                Style::new().fg(Color::Rgb {
                    r: 187,
                    g: 154,
                    b: 247,
                }), // H5: Purple
                Style::new().fg(Color::Rgb {
                    r: 125,
                    g: 207,
                    b: 255,
                }), // H6: Cyan
            ],
            backgrounds: [
                Some(Style::new().bg(Color::Rgb {
                    r: 50,
                    g: 35,
                    b: 30,
                })), // H1 bg
                Some(Style::new().bg(Color::Rgb {
                    r: 35,
                    g: 50,
                    b: 35,
                })), // H2 bg
                Some(Style::new().bg(Color::Rgb {
                    r: 30,
                    g: 35,
                    b: 50,
                })), // H3 bg
                None, // H4-H6: no background
                None,
                None,
            ],
        }
    }
}

/// Configuration for list rendering
#[derive(Debug, Clone)]
pub struct ListConfig {
    /// Bullet characters for nested levels
    pub bullets: [char; 4],
    /// Style for bullet characters
    pub bullet_style: Style,
    /// Checked checkbox symbol
    pub checkbox_checked: &'static str,
    /// Unchecked checkbox symbol
    pub checkbox_unchecked: &'static str,
    /// Checked checkbox style
    pub checkbox_checked_style: Style,
    /// Unchecked checkbox style
    pub checkbox_unchecked_style: Style,
}

impl Default for ListConfig {
    fn default() -> Self {
        Self {
            bullets: ['•', '◦', '▪', '▫'],
            bullet_style: Style::new().fg(Color::Rgb {
                r: 122,
                g: 162,
                b: 247,
            }),
            checkbox_checked: "✓",
            checkbox_unchecked: "☐",
            checkbox_checked_style: Style::new().fg(Color::Rgb {
                r: 158,
                g: 206,
                b: 106,
            }),
            checkbox_unchecked_style: Style::new().fg(Color::Rgb {
                r: 169,
                g: 177,
                b: 214,
            }),
        }
    }
}

/// Configuration for code block rendering
#[derive(Debug, Clone)]
pub struct CodeBlockConfig {
    /// Background style for code blocks
    pub background: Option<Style>,
    /// Whether to show language icons
    pub show_language_icon: bool,
}

impl Default for CodeBlockConfig {
    fn default() -> Self {
        Self {
            background: Some(Style::new().bg(Color::Rgb {
                r: 30,
                g: 30,
                b: 40,
            })),
            show_language_icon: true,
        }
    }
}

/// Configuration for inline formatting (emphasis, links)
#[derive(Debug, Clone)]
pub struct InlineConfig {
    /// Whether to conceal emphasis markers (* and **)
    pub conceal_emphasis: bool,
    /// Style for italic text (in addition to italic attribute)
    pub italic_style: Style,
    /// Style for bold text (in addition to bold attribute)
    pub bold_style: Style,
    /// Whether to conceal link syntax [text](url)
    pub conceal_links: bool,
    /// Style for link text
    pub link_style: Style,
}

impl Default for InlineConfig {
    fn default() -> Self {
        Self {
            conceal_emphasis: true,
            italic_style: Style::new().italic().fg(Color::Rgb {
                r: 169,
                g: 177,
                b: 214,
            }),
            bold_style: Style::new().bold().fg(Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            }),
            conceal_links: true,
            link_style: Style::new().underline().fg(Color::Rgb {
                r: 125,
                g: 207,
                b: 255,
            }),
        }
    }
}

/// Configuration for table rendering
#[derive(Debug, Clone)]
pub struct TableConfig {
    /// Whether to render table styling
    pub enabled: bool,
    /// Background style for header row
    pub header_background: Option<Style>,
    /// Style for header text
    pub header_style: Style,
    /// Style for delimiter row
    pub delimiter_style: Style,
    /// Style for cell borders (pipe characters)
    pub border_style: Style,
}

impl Default for TableConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            header_background: Some(Style::new().bg(Color::Rgb {
                r: 40,
                g: 40,
                b: 55,
            })),
            header_style: Style::new().bold().fg(Color::Rgb {
                r: 125,
                g: 207,
                b: 255,
            }),
            delimiter_style: Style::new().dim().fg(Color::Rgb {
                r: 100,
                g: 100,
                b: 120,
            }),
            border_style: Style::new().dim().fg(Color::Rgb {
                r: 80,
                g: 80,
                b: 100,
            }),
        }
    }
}

/// Main configuration for markdown rendering
#[derive(Debug, Clone)]
pub struct MarkdownConfig {
    /// Whether markdown rendering is enabled
    pub enabled: bool,
    /// Heading configuration
    pub headings: HeadingConfig,
    /// List configuration
    pub lists: ListConfig,
    /// Code block configuration
    pub code_blocks: CodeBlockConfig,
    /// Inline formatting configuration
    pub inline: InlineConfig,
    /// Table configuration
    pub tables: TableConfig,
    /// Show raw markdown on cursor line (even in normal mode)
    pub raw_on_cursor_line: bool,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            headings: HeadingConfig::default(),
            lists: ListConfig::default(),
            code_blocks: CodeBlockConfig::default(),
            inline: InlineConfig::default(),
            tables: TableConfig::default(),
            raw_on_cursor_line: false,
        }
    }
}

impl MarkdownConfig {
    /// Create a new default configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable markdown rendering
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set whether to show raw on cursor line
    #[must_use]
    pub const fn with_raw_on_cursor_line(mut self, raw: bool) -> Self {
        self.raw_on_cursor_line = raw;
        self
    }
}
