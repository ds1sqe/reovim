//! Formatter configuration types.

/// Configuration for an external formatter command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatterConfig {
    /// The command to run (e.g., "rustfmt", "prettier").
    pub command: String,
    /// Arguments to pass to the command.
    pub args: Vec<String>,
    /// If true, pipe content via stdin and read formatted output from stdout.
    /// If false, write to a temp file and read back (not yet implemented).
    pub stdin: bool,
}

impl FormatterConfig {
    /// Create a new formatter config for a stdin-based formatter.
    #[must_use]
    pub fn stdin(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
            stdin: true,
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
