//! Resident Set Size (RSS) measurement via `/proc/self/status`.
//!
//! Linux-only, pure `std`.  Reads the kernel-maintained status file to
//! extract `VmRSS` without shelling out or depending on `sysinfo`.
//!
//! # Example
//!
//! ```no_run
//! use reovim_bench_utils::rss;
//!
//! let before = rss::current_rss_bytes().unwrap();
//! // ... allocate something ...
//! let after = rss::current_rss_bytes().unwrap();
//! println!("delta = {} KiB", (after - before) / 1024);
//! ```

use std::io;

/// Read the current process's resident set size in **bytes**.
///
/// Parses `/proc/self/status` for the `VmRSS:` line, which the kernel
/// reports in kB (1024 bytes per unit).
///
/// Returns `Err` on non-Linux or if the file is unreadable / unparseable.
///
/// # Errors
///
/// Returns `io::Error` if `/proc/self/status` cannot be read or parsed.
pub fn current_rss_bytes() -> io::Result<u64> {
    let contents = std::fs::read_to_string("/proc/self/status")?;
    parse_vm_rss(&contents)
}

/// Parse `VmRSS` from the text of `/proc/self/status`.
///
/// Exposed for unit testing with synthetic input.
///
/// # Errors
///
/// Returns `io::Error` if the `VmRSS` line is missing or unparseable.
pub fn parse_vm_rss(status_text: &str) -> io::Result<u64> {
    for line in status_text.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let trimmed = rest.trim();
            // Expected format: "12345 kB"
            let kb_str = trimmed
                .split_whitespace()
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty VmRSS value"))?;

            let kb: u64 = kb_str.parse().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("VmRSS not a number: {e}"),
                )
            })?;

            return Ok(kb * 1024);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "VmRSS not found in /proc/self/status",
    ))
}

/// Capture RSS delta around a closure.
///
/// Returns `(result, rss_before_bytes, rss_after_bytes)`.
/// If RSS reading fails (e.g. non-Linux), the RSS values are `0`.
pub fn measure_rss<F, R>(f: F) -> (R, u64, u64)
where
    F: FnOnce() -> R,
{
    let before = current_rss_bytes().unwrap_or(0);
    let result = f();
    let after = current_rss_bytes().unwrap_or(0);
    (result, before, after)
}

#[cfg(test)]
#[path = "rss_tests.rs"]
mod tests;
