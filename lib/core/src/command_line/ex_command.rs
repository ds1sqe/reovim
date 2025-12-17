use crate::highlight::{ColorMode, ThemeName};

/// Set command options
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetOption {
    Number(bool),         // :set nu / :set nonu
    RelativeNumber(bool), // :set rnu / :set nornu
    ColorMode(ColorMode), // :set colormode=ansi|256|truecolor
    ColorScheme(ThemeName), // :colorscheme dark|light|tokyonight
    IndentGuide(bool),    // :set indentguide / :set noindentguide
    Scrollbar(bool),      // :set scrollbar / :set noscrollbar
}

/// Parsed ex-commands (colon commands)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExCommand {
    Quit,
    Write { filename: Option<String> },
    WriteQuit,
    /// Open/edit a file (:e filename)
    Edit { filename: String },
    /// Set an option (:set ...)
    Set { option: SetOption },
    /// Change colorscheme (:colorscheme name)
    Colorscheme { name: ThemeName },

    // Window management
    /// Horizontal split (:sp, :split)
    Split { filename: Option<String> },
    /// Vertical split (:vs, :vsplit)
    VSplit { filename: Option<String> },
    /// Close current window (:close, :clo)
    Close,
    /// Close all other windows (:only, :on)
    Only,

    // Tab management
    /// Create new tab (:tabnew, :tabe)
    TabNew { filename: Option<String> },
    /// Close current tab (:tabclose, :tabc)
    TabClose,
    /// Go to next tab (:tabnext, :tabn)
    TabNext,
    /// Go to previous tab (:tabprev, :tabp)
    TabPrev,

    Unknown(String),
}

impl ExCommand {
    #[must_use]
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

            _ => Some(Self::Unknown(trimmed.to_string())),
        }
    }
}
