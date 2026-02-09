//! Crash report generation.
//!
//! Linux equivalent: `kernel/panic.c` (panic message formatting)
//!
//! Generates crash reports with debugging information.

use std::{fmt::Write, panic::PanicHookInfo, path::PathBuf, time::SystemTime};

/// Format a timestamp for crash reports.
///
/// Returns `(filename_format, display_format)`:
/// - filename: `2026-02-04_12-34-56` (filesystem-safe)
/// - display: `2026-02-04 12:34:56 UTC +123456789ns`
fn format_timestamp(timestamp: SystemTime) -> (String, String) {
    let duration = timestamp
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();
    let nanos = duration.subsec_nanos();

    // Convert seconds since epoch to date/time components
    // Using simple calculation (no external crate dependency)
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;

    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year, month, day from days since 1970-01-01
    let (year, month, day) = days_to_ymd(days_since_epoch);

    let filename =
        format!("{year:04}-{month:02}-{day:02}_{hours:02}-{minutes:02}-{seconds:02}-{nanos}");

    let display = format!(
        "{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02} UTC +{nanos}ns"
    );

    (filename, display)
}

/// Convert days since Unix epoch to (year, month, day).
#[allow(
    clippy::missing_const_for_fn,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Shift to March 1, 2000 as epoch (simplifies leap year calc)
    let days = days as i64 + 719_468; // days from year 0 to 1970-01-01

    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = (days - era * 146_097) as u64; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // year of era [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month index [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };

    (y as u64, m, d)
}

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
    /// Thread name where panic occurred.
    pub thread_name: Option<String>,
    /// Thread ID where panic occurred.
    pub thread_id: Option<u64>,
    /// Server debug ring buffer dump (Phase #478).
    /// Set by server via callback during panic.
    pub server_logs: Option<String>,
    /// Paths to client debug dump files (Phase #478).
    /// Set by server via callback during panic.
    pub client_dump_paths: Vec<PathBuf>,
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
    ///
    /// # Errors
    ///
    /// Returns an error if the crash directory cannot be created or the file cannot be written.
    pub fn write_to_file(&self) -> std::io::Result<PathBuf> {
        let dir = super::recovery::recovery_dir();
        std::fs::create_dir_all(&dir)?;

        // Format timestamp for filename and display
        let (filename_ts, display_ts) = format_timestamp(self.timestamp);
        let filename = format!("crash-{filename_ts}.txt");
        let path = dir.join(&filename);

        let mut content = format!(
            "Reovim Crash Report\n\
==================\n\n\
Timestamp: {}\n\
Reovim Version: {}\n\
Rust Version: {}\n\
Thread: {} (id: {})\n\n\
Panic Message:\n{}\n\n\
Location:\n{}\n\n\
Backtrace:\n{}\n",
            display_ts,
            self.reovim_version,
            self.rust_version,
            self.thread_name.as_deref().unwrap_or("unnamed"),
            self.thread_id
                .map_or_else(|| "unknown".to_string(), |id| id.to_string()),
            self.panic_message,
            self.panic_location.as_deref().unwrap_or("unknown"),
            self.backtrace
        );

        // Append server logs if available
        if let Some(ref logs) = self.server_logs {
            content.push_str("\nServer Debug Logs:\n");
            content.push_str("------------------\n");
            content.push_str(logs);
            content.push('\n');
        }

        // Append client dump paths if available
        if !self.client_dump_paths.is_empty() {
            content.push_str("\nClient Debug Dumps:\n");
            content.push_str("-------------------\n");
            for path in &self.client_dump_paths {
                let _ = writeln!(content, "  - {}", path.display());
            }
        }

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

    // Get thread info
    let current_thread = std::thread::current();
    let thread_name = current_thread.name().map(ToString::to_string);
    // ThreadId doesn't expose the raw value on stable, so we use Debug format
    let thread_id_str = format!("{:?}", current_thread.id());
    // Parse the thread ID from "ThreadId(N)" format
    let thread_id = thread_id_str
        .strip_prefix("ThreadId(")
        .and_then(|s| s.strip_suffix(')'))
        .and_then(|s| s.parse::<u64>().ok());

    CrashReport {
        timestamp: std::time::SystemTime::now(),
        panic_message,
        panic_location,
        backtrace,
        rust_version: option_env!("RUSTC_VERSION").unwrap_or("unknown"),
        reovim_version: env!("CARGO_PKG_VERSION"),
        thread_name,
        thread_id,
        // These are populated by server via callback before write_to_file()
        server_logs: None,
        client_dump_paths: Vec::new(),
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
            thread_name: Some("main".to_string()),
            thread_id: Some(1),
            server_logs: None,
            client_dump_paths: Vec::new(),
        };

        let summary = report.summary();
        assert!(summary.contains("test panic"));
        assert!(summary.contains("src/main.rs:42:5"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_crash_report_write_to_file() {
        let report = CrashReport {
            timestamp: std::time::SystemTime::now(),
            panic_message: "test panic for file".to_string(),
            panic_location: Some("test.rs:1:1".to_string()),
            backtrace: "test backtrace".to_string(),
            rust_version: "1.80.0",
            reovim_version: "0.9.0-test",
            thread_name: Some("test-thread".to_string()),
            thread_id: Some(42),
            server_logs: None,
            client_dump_paths: Vec::new(),
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

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_crash_report_with_server_logs() {
        let report = CrashReport {
            timestamp: std::time::SystemTime::now(),
            panic_message: "test panic".to_string(),
            panic_location: Some("test.rs:1:1".to_string()),
            backtrace: "backtrace".to_string(),
            rust_version: "1.80.0",
            reovim_version: "0.9.0",
            thread_name: Some("main".to_string()),
            thread_id: Some(1),
            server_logs: Some("INFO: server started\nERROR: connection lost".to_string()),
            client_dump_paths: vec![
                PathBuf::from("/tmp/client-1.log"),
                PathBuf::from("/tmp/client-2.log"),
            ],
        };

        let result = report.write_to_file();
        if result.is_err() {
            eprintln!("Skipping test: recovery dir not writable");
            return;
        }

        let path = result.unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        assert!(content.contains("Server Debug Logs:"));
        assert!(content.contains("INFO: server started"));
        assert!(content.contains("Client Debug Dumps:"));
        assert!(content.contains("/tmp/client-1.log"));
        assert!(content.contains("/tmp/client-2.log"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_crash_report_thread_info() {
        let report = CrashReport {
            timestamp: std::time::SystemTime::now(),
            panic_message: "test".to_string(),
            panic_location: None,
            backtrace: String::new(),
            rust_version: "1.80.0",
            reovim_version: "0.9.0",
            thread_name: Some("worker-42".to_string()),
            thread_id: Some(42),
            server_logs: None,
            client_dump_paths: Vec::new(),
        };

        let result = report.write_to_file();
        if result.is_err() {
            return;
        }

        let path = result.unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        assert!(content.contains("Thread: worker-42"));
        assert!(content.contains("id: 42"));

        std::fs::remove_file(&path).ok();
    }
}
