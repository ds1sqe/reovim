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
    /// Spaces to indent per heading level (H2+)
    /// H1 = 0 spaces, H2 = 1*indent, H3 = 2*indent, etc.
    pub indent_per_level: u8,
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
            indent_per_level: 0,
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
    /// Style for language name/icon
    pub lang_style: Style,
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
            lang_style: Style::new().fg(Color::Rgb {
                r: 125,
                g: 207,
                b: 255,
            }),
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
    /// Optional background for links (applied in addition to link_style)
    pub link_background: Option<Style>,
    /// Style for inline code spans (`code`)
    pub code_span_style: Style,
    /// Whether to conceal the backtick markers
    pub conceal_code_span: bool,
    /// Style for strikethrough text (~~text~~)
    pub strikethrough_style: Style,
    /// Whether to conceal ~~ markers
    pub conceal_strikethrough: bool,
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
            link_background: Some(Style::new().bg(Color::Rgb {
                r: 30,
                g: 40,
                b: 50,
            })),
            code_span_style: Style::new()
                .bg(Color::Rgb {
                    r: 45,
                    g: 45,
                    b: 55,
                })
                .fg(Color::Rgb {
                    r: 230,
                    g: 180,
                    b: 120,
                }),
            conceal_code_span: true,
            strikethrough_style: Style::new().strikethrough().dim().fg(Color::Rgb {
                r: 140,
                g: 140,
                b: 160,
            }),
            conceal_strikethrough: true,
        }
    }
}

/// Box-drawing characters for table borders
#[derive(Debug, Clone)]
pub struct BoxDrawingChars {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
    pub cross: char,
    pub top_tee: char,
    pub bottom_tee: char,
    pub left_tee: char,
    pub right_tee: char,
}

impl Default for BoxDrawingChars {
    fn default() -> Self {
        Self {
            top_left: '┌',
            top_right: '┐',
            bottom_left: '└',
            bottom_right: '┘',
            horizontal: '─',
            vertical: '│',
            cross: '┼',
            top_tee: '┬',
            bottom_tee: '┴',
            left_tee: '├',
            right_tee: '┤',
        }
    }
}

/// Configuration for table border rendering with box-drawing characters
#[derive(Debug, Clone)]
pub struct TableBorderConfig {
    /// Whether to use box-drawing characters for borders
    pub use_box_drawing: bool,
    /// Box-drawing character set
    pub chars: BoxDrawingChars,
    /// Style for border characters
    pub style: Style,
}

impl Default for TableBorderConfig {
    fn default() -> Self {
        Self {
            use_box_drawing: true, // Enabled by default for visual polish
            chars: BoxDrawingChars::default(),
            style: Style::new().dim().fg(Color::Rgb {
                r: 80,
                g: 80,
                b: 100,
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
    /// Box-drawing border configuration
    pub borders: TableBorderConfig,
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
            borders: TableBorderConfig::default(),
        }
    }
}

/// Configuration for horizontal rule rendering
#[derive(Debug, Clone)]
pub struct HorizontalRuleConfig {
    /// Whether to render horizontal rule styling
    pub enabled: bool,
    /// Character to use for the rule (repeated to fill width)
    pub character: char,
    /// Style for the horizontal rule
    pub style: Style,
    /// Minimum width of the rule (in characters)
    pub min_width: u16,
}

impl Default for HorizontalRuleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            character: '─', // Box-drawing horizontal
            style: Style::new().dim().fg(Color::Rgb {
                r: 100,
                g: 100,
                b: 120,
            }),
            min_width: 40,
        }
    }
}

/// Configuration for blockquote rendering
#[derive(Debug, Clone)]
pub struct BlockquoteConfig {
    /// Whether to render blockquote styling
    pub enabled: bool,
    /// Replacement character for > marker
    pub marker: &'static str,
    /// Style for the marker
    pub marker_style: Style,
    /// Optional background for the entire blockquote
    pub background: Option<Style>,
}

impl Default for BlockquoteConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            marker: "│ ", // Vertical line + space
            marker_style: Style::new().fg(Color::Rgb {
                r: 100,
                g: 140,
                b: 180,
            }),
            background: Some(Style::new().bg(Color::Rgb {
                r: 35,
                g: 40,
                b: 48,
            })),
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
    /// Horizontal rule configuration
    pub horizontal_rules: HorizontalRuleConfig,
    /// Blockquote configuration
    pub blockquotes: BlockquoteConfig,
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
            horizontal_rules: HorizontalRuleConfig::default(),
            blockquotes: BlockquoteConfig::default(),
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
