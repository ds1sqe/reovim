//! Crash recovery utilities.
//!
//! Linux equivalent: `kernel/panic.c` `emergency_restart`
//!
//! Provides utilities to save state during panic and recover after restart.

use std::{fmt::Write, path::PathBuf};

/// Recovery state snapshot.
#[derive(Debug)]
pub struct RecoverySnapshot {
    /// Unsaved buffers at time of crash.
    pub unsaved_buffers: Vec<UnsavedBuffer>,
    /// Timestamp of the crash.
    pub timestamp: std::time::SystemTime,
}

/// Information about an unsaved buffer.
#[derive(Debug)]
pub struct UnsavedBuffer {
    /// Buffer ID.
    pub id: u32,
    /// Original file path (if any).
    pub path: Option<PathBuf>,
    /// Hash of content for deduplication.
    pub content_hash: u64,
    /// Number of lines.
    pub line_count: usize,
}

/// Get the recovery data directory.
///
/// Returns `~/.local/share/reovim/recovery/` or equivalent.
#[must_use]
pub fn recovery_dir() -> PathBuf {
    reovim_arch::dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("reovim")
        .join("recovery")
}

/// Save buffer content to recovery directory.
///
/// Creates a recovery file with metadata header and buffer content.
///
/// # Arguments
///
/// * `buffer_id` - Numeric buffer ID
/// * `original_path` - Original file path (if any)
/// * `content` - Buffer content
///
/// # Returns
///
/// Path to the recovery file, or an error.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create the recovery directory
/// - Unable to write the recovery file
///
/// # Example
///
/// ```ignore
/// let path = save_buffer_for_recovery(1, Some(Path::new("main.rs")), "fn main() {}");
/// println!("Saved to: {}", path?.display());
/// ```
pub fn save_buffer_for_recovery(
    buffer_id: u32,
    original_path: Option<&std::path::Path>,
    content: &str,
) -> std::io::Result<PathBuf> {
    let dir = recovery_dir();
    std::fs::create_dir_all(&dir)?;

    let filename = format!(
        "buffer-{}-{}.txt",
        buffer_id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    );

    let path = dir.join(&filename);

    // Write metadata header
    let mut output = String::new();
    writeln!(output, "# Recovery file for buffer {buffer_id}").ok();
    if let Some(orig) = original_path {
        writeln!(output, "# Original path: {}", orig.display()).ok();
    }
    writeln!(output, "# Lines: {}", content.lines().count()).ok();
    output.push_str("# ---\n");
    output.push_str(content);

    std::fs::write(&path, output)?;
    Ok(path)
}

/// List all recovery files.
///
/// # Returns
///
/// List of paths to recovery files, sorted by modification time (newest first).
///
/// # Errors
///
/// Returns an error if unable to read the recovery directory.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn list_recovery_files() -> std::io::Result<Vec<PathBuf>> {
    let dir = recovery_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "txt")
            && let Ok(meta) = std::fs::metadata(&path)
            && let Ok(modified) = meta.modified()
        {
            files.push((path, modified));
        }
    }

    // Sort by modification time (newest first)
    files.sort_by_key(|b| std::cmp::Reverse(b.1));

    Ok(files.into_iter().map(|(p, _)| p).collect())
}

/// Clean up old recovery files.
///
/// Removes recovery files older than the specified age.
///
/// # Arguments
///
/// * `max_age_secs` - Maximum age in seconds
///
/// # Returns
///
/// Number of files removed.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to list recovery files
/// - Unable to remove a file
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn cleanup_old_recovery_files(max_age_secs: u64) -> std::io::Result<usize> {
    let now = std::time::SystemTime::now();
    let mut removed = 0;

    for path in list_recovery_files()? {
        if let Ok(meta) = std::fs::metadata(&path)
            && let Ok(modified) = meta.modified()
            && let Ok(age) = now.duration_since(modified)
            && age.as_secs() > max_age_secs
        {
            std::fs::remove_file(&path)?;
            removed += 1;
        }
    }

    Ok(removed)
}

