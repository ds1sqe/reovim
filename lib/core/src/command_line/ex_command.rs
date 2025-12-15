/// Parsed ex-commands (colon commands)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExCommand {
    Quit,
    Write { filename: Option<String> },
    WriteQuit,
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
            _ => Some(ExCommand::Unknown(trimmed.to_string())),
        }
    }
}
