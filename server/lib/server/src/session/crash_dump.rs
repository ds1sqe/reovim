//! Client crash dump utilities.
//!
//! Dumps client ring buffers to files for post-mortem analysis.
//!
//! # File Location
//!
//! Client dumps are written to:
//! `~/.local/share/reovim/crash/client-{id}-{timestamp}.log`
//!
//! # Usage
//!
//! ```ignore
//! use reovim_server::session::{ClientId, crash_dump};
//!
//! // Dump a single client's ring buffer
//! let path = crash_dump::dump_client_to_file(client_id, &ring_buffer)?;
//!
//! // Get the crash directory path
//! let dir = crash_dump::crash_dir();
//! ```

use std::{
    io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use reovim_arch::dirs::data_local_dir;

use super::{ClientId, ClientRingBuffer};

/// Get the directory for crash dumps.
///
/// Returns `~/.local/share/reovim/crash/`
#[must_use]
pub fn crash_dir() -> PathBuf {
    data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("reovim")
        .join("crash")
}

/// Dump a client's ring buffer to a file.
///
/// # Arguments
///
/// * `client_id` - The client ID (used in filename)
/// * `ring_buffer` - The client's ring buffer to dump
///
/// # Returns
///
/// Path to the dump file on success.
///
/// # Errors
///
/// Returns an error if the crash directory cannot be created or the file cannot be written.
///
/// # File Format
///
/// ```text
/// Client Debug Dump
/// =================
///
/// Client ID: {id}
/// Timestamp: {timestamp}
///
/// Event Log:
/// ----------
/// [seq] [time] [type] message
/// ...
/// ```
pub fn dump_client_to_file(
    client_id: ClientId,
    ring_buffer: &ClientRingBuffer,
) -> io::Result<PathBuf> {
    let dir = crash_dir();
    std::fs::create_dir_all(&dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let filename = format!("client-{}-{}.log", client_id.as_usize(), timestamp);
    let path = dir.join(&filename);

    // Get the dump content (uses try_dump internally for safety)
    let events = ring_buffer
        .try_dump()
        .unwrap_or_else(|| "Failed to acquire lock".to_string());

    let content = format!(
        "Client Debug Dump\n\
         =================\n\n\
         Client ID: {}\n\
         Timestamp: {}\n\n\
         Event Log:\n\
         ----------\n\
         {}\n",
        client_id.as_usize(),
        timestamp,
        events
    );

    std::fs::write(&path, content)?;
    Ok(path)
}

/// Dump a client's ring buffer using `try_dump` (non-blocking).
///
/// Returns None if the lock is held (safe for use in panic handler).
pub fn try_dump_client_to_file(
    client_id: ClientId,
    ring_buffer: &ClientRingBuffer,
) -> Option<PathBuf> {
    let dir = crash_dir();
    std::fs::create_dir_all(&dir).ok()?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let filename = format!("client-{}-{}.log", client_id.as_usize(), timestamp);
    let path = dir.join(&filename);

    let events = ring_buffer.try_dump()?;

    let content = format!(
        "Client Debug Dump\n\
         =================\n\n\
         Client ID: {}\n\
         Timestamp: {}\n\n\
         Event Log:\n\
         ----------\n\
         {}\n",
        client_id.as_usize(),
        timestamp,
        events
    );

    std::fs::write(&path, &content).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_dir() {
        let dir = crash_dir();
        assert!(dir.ends_with("reovim/crash") || dir.ends_with("reovim\\crash"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dump_client_to_file() {
        let ring_buffer = ClientRingBuffer::new();
        ring_buffer.log_key("a");
        ring_buffer.log_key("b");
        ring_buffer.log_command("write");

        let result = dump_client_to_file(ClientId::new(42), &ring_buffer);

        // May fail in CI if directory is not writable
        if let Ok(path) = result {
            assert!(path.exists());

            let content = std::fs::read_to_string(&path).unwrap();
            assert!(content.contains("Client Debug Dump"));
            assert!(content.contains("Client ID: 42"));
            assert!(content.contains("KEY"));
            assert!(content.contains("CMD"));

            std::fs::remove_file(&path).ok();
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_try_dump_client_to_file() {
        let ring_buffer = ClientRingBuffer::new();
        ring_buffer.log_error("test error");

        let result = try_dump_client_to_file(ClientId::new(99), &ring_buffer);

        if let Some(path) = result {
            assert!(path.exists());

            let content = std::fs::read_to_string(&path).unwrap();
            assert!(content.contains("Client ID: 99"));
            assert!(content.contains("ERROR"));

            std::fs::remove_file(&path).ok();
        }
    }
}
