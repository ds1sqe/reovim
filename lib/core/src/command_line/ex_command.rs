use crate::highlight::{ColorMode, ThemeName};

/// Set command options
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetOption {
    // === Core options (fast path) ===
    Number(bool),           // :set nu / :set nonu
    RelativeNumber(bool),   // :set rnu / :set nornu
    ColorMode(ColorMode),   // :set colormode=ansi|256|truecolor
    ColorScheme(ThemeName), // :colorscheme dark|light|tokyonight
    IndentGuide(bool),      // :set indentguide / :set noindentguide
    Scrollbar(bool),        // :set scrollbar / :set noscrollbar

    // === Dynamic options (from OptionRegistry) ===
    /// Enable a boolean option: `:set optionname`
    DynamicEnable(String),

    /// Disable a boolean option: `:set nooptionname`
    DynamicDisable(String),

    /// Set option to a value: `:set optionname=value`
    DynamicSet {
        name: String,
        value: String,
    },

    /// Query option value: `:set optionname?`
    DynamicQuery(String),

    /// Reset to default: `:set optionname&`
    DynamicReset(String),
}

/// Parsed ex-commands (colon commands)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExCommand {
    Quit,
    Write {
        filename: Option<String>,
    },
    WriteQuit,
    /// Open/edit a file (:e filename)
    Edit {
        filename: String,
    },
    /// Set an option (:set ...)
    Set {
        option: SetOption,
    },
    /// Change colorscheme (:colorscheme name)
    Colorscheme {
        name: ThemeName,
    },

    // Window management
    /// Horizontal split (:sp, :split)
    Split {
        filename: Option<String>,
    },
    /// Vertical split (:vs, :vsplit)
    VSplit {
        filename: Option<String>,
    },
    /// Close current window (:close, :clo)
    Close,
    /// Close all other windows (:only, :on)
    Only,

    // Tab management
    /// Create new tab (:tabnew, :tabe)
    TabNew {
        filename: Option<String>,
    },
    /// Close current tab (:tabclose, :tabc)
    TabClose,
    /// Go to next tab (:tabnext, :tabn)
    TabNext,
    /// Go to previous tab (:tabprev, :tabp)
    TabPrev,

    /// Plugin-registered ex-command (dispatched via registry)
    Plugin {
        command: String,
    },

    Unknown(String),
}

impl ExCommand {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }

        match trimmed {
            "q" | "quit" => Some(Self::Quit),
            "w" | "write" => Some(Self::Write { filename: None }),
            "wq" => Some(Self::WriteQuit),
            s if s.starts_with("w ") => Some(Self::Write {
                filename: Some(s[2..].trim().to_string()),
            }),
            // :e or :edit commands for opening files
            s if s.starts_with("e ") => Some(Self::Edit {
                filename: s[2..].trim().to_string(),
            }),
            s if s.starts_with("edit ") => Some(Self::Edit {
                filename: s[5..].trim().to_string(),
            }),
            // :set commands for line numbers
            "set nu" | "set number" => Some(Self::Set {
                option: SetOption::Number(true),
            }),
            "set nonu" | "set nonumber" => Some(Self::Set {
                option: SetOption::Number(false),
            }),
            "set rnu" | "set relativenumber" => Some(Self::Set {
                option: SetOption::RelativeNumber(true),
            }),
            "set nornu" | "set norelativenumber" => Some(Self::Set {
                option: SetOption::RelativeNumber(false),
            }),
            // :set indentguide / :set noindentguide
            "set indentguide" | "set ig" => Some(Self::Set {
                option: SetOption::IndentGuide(true),
            }),
            "set noindentguide" | "set noig" => Some(Self::Set {
                option: SetOption::IndentGuide(false),
            }),
            // :set scrollbar / :set noscrollbar
            "set scrollbar" | "set sb" => Some(Self::Set {
                option: SetOption::Scrollbar(true),
            }),
            "set noscrollbar" | "set nosb" => Some(Self::Set {
                option: SetOption::Scrollbar(false),
            }),
            // :set colormode=ansi|256|truecolor
            s if s.starts_with("set colormode=") => {
                let mode_str = &s[14..];
                ColorMode::parse(mode_str).map(|mode| Self::Set {
                    option: SetOption::ColorMode(mode),
                })
            }

            // Dynamic :set commands (plugin options, etc.)
            s if s.starts_with("set ") => Self::parse_dynamic_set(&s[4..]),

            // :colorscheme name
            s if s.starts_with("colorscheme ") || s.starts_with("colo ") => {
                let name_start = if s.starts_with("colorscheme ") { 12 } else { 5 };
                let name_str = s[name_start..].trim();
                ThemeName::parse(name_str).map(|name| Self::Colorscheme { name })
            }

            // Window management
            "sp" | "split" => Some(Self::Split { filename: None }),
            s if s.starts_with("sp ") => Some(Self::Split {
                filename: Some(s[3..].trim().to_string()),
            }),
            s if s.starts_with("split ") => Some(Self::Split {
                filename: Some(s[6..].trim().to_string()),
            }),
            "vs" | "vsplit" => Some(Self::VSplit { filename: None }),
            s if s.starts_with("vs ") => Some(Self::VSplit {
                filename: Some(s[3..].trim().to_string()),
            }),
            s if s.starts_with("vsplit ") => Some(Self::VSplit {
                filename: Some(s[7..].trim().to_string()),
            }),
            "close" | "clo" => Some(Self::Close),
            "only" | "on" => Some(Self::Only),

            // Tab management
            "tabnew" | "tabe" => Some(Self::TabNew { filename: None }),
            s if s.starts_with("tabnew ") => Some(Self::TabNew {
                filename: Some(s[7..].trim().to_string()),
            }),
            s if s.starts_with("tabe ") => Some(Self::TabNew {
                filename: Some(s[5..].trim().to_string()),
            }),
            "tabclose" | "tabc" => Some(Self::TabClose),
            "tabnext" | "tabn" => Some(Self::TabNext),
            "tabprev" | "tabp" => Some(Self::TabPrev),

            _ => Some(Self::Plugin {
                command: trimmed.to_string(),
            }),
        }
    }

    /// Parse dynamic :set commands for plugin options
    ///
    /// Handles formats:
    /// - `:set optionname` - enable boolean
    /// - `:set nooptionname` - disable boolean
    /// - `:set optionname=value` - set value
    /// - `:set optionname?` - query value
    /// - `:set optionname&` - reset to default
    fn parse_dynamic_set(option_str: &str) -> Option<Self> {
        let option_str = option_str.trim();
        if option_str.is_empty() {
            return None;
        }

        // Query: :set option?
        if let Some(name) = option_str.strip_suffix('?') {
            return Some(Self::Set {
                option: SetOption::DynamicQuery(name.to_string()),
            });
        }

        // Reset: :set option&
        if let Some(name) = option_str.strip_suffix('&') {
            return Some(Self::Set {
                option: SetOption::DynamicReset(name.to_string()),
            });
        }

        // Set value: :set option=value
        if let Some(eq_pos) = option_str.find('=') {
            let name = option_str[..eq_pos].trim();
            let value = option_str[eq_pos + 1..].trim();
            return Some(Self::Set {
                option: SetOption::DynamicSet {
                    name: name.to_string(),
                    value: value.to_string(),
                },
            });
        }

        // Disable: :set nooption
        if let Some(name) = option_str.strip_prefix("no") {
            // Avoid matching things like "normal" - require at least 2 chars after "no"
            if name.len() >= 2 {
                return Some(Self::Set {
                    option: SetOption::DynamicDisable(name.to_string()),
                });
            }
        }

        // Enable: :set option (boolean enable)
        Some(Self::Set {
            option: SetOption::DynamicEnable(option_str.to_string()),
        })
    }
}
