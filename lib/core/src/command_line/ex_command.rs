/// Set command options
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetOption {
    Number(bool),         // :set nu / :set nonu
    RelativeNumber(bool), // :set rnu / :set nornu
}

/// Parsed ex-commands (colon commands)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExCommand {
    Quit,
    Write { filename: Option<String> },
    WriteQuit,
    Set { option: SetOption },
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
            _ => Some(Self::Unknown(trimmed.to_string())),
        }
    }
}
