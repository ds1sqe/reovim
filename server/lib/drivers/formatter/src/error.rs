//! Format error types.

use std::fmt;

/// Errors from formatting operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// External command failed with non-zero exit code.
    CommandFailed {
        /// The command that was run.
        command: String,
        /// Stderr output.
        stderr: String,
        /// Exit code (None if killed by signal).
        exit_code: Option<i32>,
    },
    /// External command not found on PATH.
    CommandNotFound(String),
    /// Formatting timed out.
    Timeout,
    /// LSP formatting error.
    LspError(String),
    /// Content processing error.
    ContentError(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandFailed {
                command,
                stderr,
                exit_code,
            } => {
                write!(f, "formatter '{command}' failed")?;
                if let Some(code) = exit_code {
                    write!(f, " (exit {code})")?;
                }
                if !stderr.is_empty() {
                    write!(f, ": {stderr}")?;
                }
                Ok(())
            }
            Self::CommandNotFound(cmd) => write!(f, "formatter not found: {cmd}"),
            Self::Timeout => write!(f, "formatting timed out"),
            Self::LspError(msg) => write!(f, "LSP formatting error: {msg}"),
            Self::ContentError(msg) => write!(f, "content error: {msg}"),
        }
    }
}

impl std::error::Error for FormatError {}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
