//! Crash report generation.
//!
//! Linux equivalent: `kernel/panic.c` (panic message formatting)
//!
//! Generates crash reports with debugging information.

use std::{panic::PanicHookInfo, path::PathBuf};

/// Crash report with all debugging info.
#[derive(Debug)]
pub struct CrashReport {
    /// Timestamp of the crash.
    pub timestamp: std::time::SystemTime,
    /// Panic message.
    pub panic_message: String,
    /// Source location of the panic.
    pub panic_location: Option<String>,
    /// Stack backtrace.
    pub backtrace: String,
    /// Rust compiler version.
    pub rust_version: &'static str,
    /// Reovim version.
    pub reovim_version: &'static str,
}

impl CrashReport {
    /// Get a short summary of the crash.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} at {}",
            self.panic_message,
            self.panic_location.as_deref().unwrap_or("unknown location")
        )
    }

    /// Write the crash report to a file.
    ///
    /// # Returns
    ///
    /// Path to the crash report file.
    pub fn write_to_file(&self) -> std::io::Result<PathBuf> {
        let dir = super::recovery::recovery_dir();
        std::fs::create_dir_all(&dir)?;

        let filename = format!(
            "crash-{}.txt",
            self.timestamp
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        );
        let path = dir.join(&filename);

        let content = format!(
            "Reovim Crash Report\n\
             ==================\n\n\
             Timestamp: {:?}\n\
             Reovim Version: {}\n\
             Rust Version: {}\n\n\
             Panic Message:\n{}\n\n\
             Location:\n{}\n\n\
             Backtrace:\n{}\n",
            self.timestamp,
            self.reovim_version,
            self.rust_version,
            self.panic_message,
            self.panic_location.as_deref().unwrap_or("unknown"),
            self.backtrace
        );

        std::fs::write(&path, content)?;
        Ok(path)
    }
}

/// Generate a crash report from panic info.
///
/// # Arguments
///
/// * `info` - Panic hook info from the panic handler
///
/// # Returns
///
/// A `CrashReport` with all available debugging information.
#[must_use]
pub fn generate_crash_report(info: &PanicHookInfo<'_>) -> CrashReport {
    let panic_message = info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .map(ToString::to_string)
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "Unknown panic".to_string());

    let panic_location = info
        .location()
        .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));

    let backtrace = std::backtrace::Backtrace::force_capture().to_string();

    CrashReport {
        timestamp: std::time::SystemTime::now(),
        panic_message,
        panic_location,
        backtrace,
        rust_version: option_env!("RUSTC_VERSION").unwrap_or("unknown"),
        reovim_version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_report_summary() {
        let report = CrashReport {
            timestamp: std::time::SystemTime::now(),
            panic_message: "test panic".to_string(),
            panic_location: Some("src/main.rs:42:5".to_string()),
            backtrace: "backtrace...".to_string(),
            rust_version: "1.80.0",
            reovim_version: "0.9.0",
        };

        let summary = report.summary();
        assert!(summary.contains("test panic"));
        assert!(summary.contains("src/main.rs:42:5"));
    }

    #[test]
    fn test_crash_report_write_to_file() {
        let report = CrashReport {
            timestamp: std::time::SystemTime::now(),
            panic_message: "test panic for file".to_string(),
            panic_location: Some("test.rs:1:1".to_string()),
            backtrace: "test backtrace".to_string(),
            rust_version: "1.80.0",
            reovim_version: "0.9.0-test",
        };

        // Try to write - may fail in CI if HOME is not set
        let result = report.write_to_file();

        // Skip test if directory is not writable (CI environment)
        if result.is_err() {
            eprintln!("Skipping test_crash_report_write_to_file: recovery dir not writable");
            return;
        }

        let path = result.unwrap();
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Reovim Crash Report"));
        assert!(content.contains("test panic for file"));
        assert!(content.contains("test backtrace"));

        // Cleanup
        std::fs::remove_file(&path).ok();
    }
}
