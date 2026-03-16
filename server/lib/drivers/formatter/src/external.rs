//! External formatter — runs CLI commands.

use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use tracing::warn;

use crate::{config::FormatterConfig, error::FormatError, provider::FormatterProvider};

/// Default timeout for external formatters (5 seconds).
const DEFAULT_TIMEOUT_SECS: u64 = 5;

/// Formatter that runs an external CLI command.
///
/// Pipes buffer content via stdin, captures formatted output from stdout.
pub struct ExternalFormatter {
    config: FormatterConfig,
    timeout: Duration,
}

impl ExternalFormatter {
    /// Create a new external formatter from config.
    #[must_use]
    pub const fn new(config: FormatterConfig) -> Self {
        Self {
            config,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    /// Create with a custom timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl FormatterProvider for ExternalFormatter {
    fn format(&self, content: &str, path: &Path) -> Result<String, FormatError> {
        let mut args = self.config.args.clone();

        // Some formatters need the file path for filetype detection
        // (e.g., prettierd --stdin-filepath <path>)
        let path_str = path.display().to_string();
        for arg in &mut args {
            #[allow(clippy::literal_string_with_formatting_args)]
            if arg.contains("{path}") {
                *arg = arg.replace("{path}", &path_str);
            }
        }

        let mut child = Command::new(&self.config.command)
            .args(&args)
            .stdin(if self.config.stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    FormatError::CommandNotFound(self.config.command.clone())
                } else {
                    FormatError::CommandFailed {
                        command: self.config.command.clone(),
                        stderr: e.to_string(),
                        exit_code: None,
                    }
                }
            })?;

        // Write content to stdin
        if self.config.stdin
            && let Some(mut stdin) = child.stdin.take()
        {
            stdin
                .write_all(content.as_bytes())
                .map_err(|e| FormatError::CommandFailed {
                    command: self.config.command.clone(),
                    stderr: format!("stdin write failed: {e}"),
                    exit_code: None,
                })?;
        }

        // Wait for completion with timeout enforcement.
        // Move child to a thread and use channel recv_timeout to enforce the deadline.
        let timeout = self.timeout;
        let cmd_name = self.config.command.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });

        let output = match rx.recv_timeout(timeout) {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                warn!(command = %cmd_name, error = %e, "Formatter wait failed");
                return Err(FormatError::CommandFailed {
                    command: cmd_name,
                    stderr: e.to_string(),
                    exit_code: None,
                });
            }
            Err(_) => {
                warn!(command = %cmd_name, timeout_secs = timeout.as_secs(), "Formatter timed out");
                return Err(FormatError::Timeout);
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(FormatError::CommandFailed {
                command: self.config.command.clone(),
                stderr,
                exit_code: output.status.code(),
            });
        }

        String::from_utf8(output.stdout).map_err(|e| {
            FormatError::ContentError(format!("formatter output not valid UTF-8: {e}"))
        })
    }

    fn name(&self) -> &str {
        &self.config.command
    }
}

#[cfg(test)]
#[path = "external_tests.rs"]
mod tests;
