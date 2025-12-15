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
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }

        match trimmed {
            "q" | "quit" => Some(ExCommand::Quit),
            "w" | "write" => Some(ExCommand::Write { filename: None }),
            "wq" => Some(ExCommand::WriteQuit),
            s if s.starts_with("w ") => Some(ExCommand::Write {
                filename: Some(s[2..].trim().to_string()),
            }),
            // :set commands for line numbers
            "set nu" | "set number" => Some(ExCommand::Set {
                option: SetOption::Number(true),
            }),
            "set nonu" | "set nonumber" => Some(ExCommand::Set {
                option: SetOption::Number(false),
            }),
            "set rnu" | "set relativenumber" => Some(ExCommand::Set {
                option: SetOption::RelativeNumber(true),
            }),
            "set nornu" | "set norelativenumber" => Some(ExCommand::Set {
                option: SetOption::RelativeNumber(false),
            }),
            _ => Some(ExCommand::Unknown(trimmed.to_string())),
        }
    }
}
