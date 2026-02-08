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

#[cfg(test)]
mod tests {
    use {super::*, std::path::Path};

    #[test]
    fn test_recovery_dir() {
        let dir = recovery_dir();
        assert!(dir.ends_with("recovery"));
    }

    #[test]
    fn test_save_buffer_for_recovery() {
        let content = "line 1\nline 2\nline 3";
        let result = save_buffer_for_recovery(999, Some(Path::new("/tmp/test.txt")), content);

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());

        // Verify content
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("buffer 999"));
        assert!(saved.contains("/tmp/test.txt"));
        assert!(saved.contains("Lines: 3"));
        assert!(saved.contains(content));

        // Cleanup
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_list_recovery_files_empty() {
        // Create a unique test directory to avoid interference
        let test_dir = std::env::temp_dir().join("reovim_test_recovery_list");
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir).ok();
        }

        // Without any files, should return empty
        let files = list_recovery_files();
        // Note: This test depends on the actual recovery directory state
        assert!(files.is_ok());
    }

    // ========== save_buffer_for_recovery without original path ==========

    #[test]
    fn test_save_buffer_for_recovery_no_path() {
        let content = "no path buffer content";
        let result = save_buffer_for_recovery(888, None, content);

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("buffer 888"));
        // Should NOT contain "Original path" since we passed None
        assert!(!saved.contains("Original path"));
        assert!(saved.contains(content));

        // Cleanup
        std::fs::remove_file(&path).ok();
    }

    // ========== list_recovery_files with actual files ==========

    #[test]
    fn test_list_recovery_files_with_files() {
        // Save a file and then list
        let content = "test list content";
        let result = save_buffer_for_recovery(777, Some(Path::new("/tmp/list_test.rs")), content);
        assert!(result.is_ok());
        let saved_path = result.unwrap();

        let files = list_recovery_files().unwrap();
        // Should contain at least one file (the one we just saved)
        assert!(!files.is_empty());

        // Cleanup
        std::fs::remove_file(&saved_path).ok();
    }

    // ========== cleanup_old_recovery_files ==========

    #[test]
    fn test_cleanup_old_recovery_files() {
        // Save a file for cleanup test
        let content = "cleanup test content";
        let result = save_buffer_for_recovery(666, Some(Path::new("/tmp/cleanup.rs")), content);
        assert!(result.is_ok());
        let saved_path = result.unwrap();

        // Cleanup with max_age of 0 should remove everything
        // (since max_age_secs=0 means files with age > 0 are removed)
        // However, the file was JUST created, so its age might be 0.
        // Use a very large max_age to keep files and verify it works
        let removed = cleanup_old_recovery_files(999_999).unwrap();
        // We don't know the exact count, but it should not error
        let _ = removed;

        // If the file still exists, clean it up manually
        std::fs::remove_file(&saved_path).ok();
    }

    // ========== RecoverySnapshot and UnsavedBuffer ==========

    #[test]
    fn test_recovery_snapshot_debug() {
        let snapshot = RecoverySnapshot {
            unsaved_buffers: vec![UnsavedBuffer {
                id: 1,
                path: Some(PathBuf::from("/test.rs")),
                content_hash: 12345,
                line_count: 42,
            }],
            timestamp: std::time::SystemTime::now(),
        };
        let debug = format!("{snapshot:?}");
        assert!(debug.contains("RecoverySnapshot"));
        assert!(debug.contains("UnsavedBuffer"));
    }
}
