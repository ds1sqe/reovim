//! Hunk navigation commands.
//!
//! `]h` and `[h` jump to the next/previous diff hunk in the current buffer.

use std::path::Path;

use reovim_subsys_git::types::DiffHunk;

/// Find the index of the next hunk after `cursor_line` (0-indexed).
///
/// Returns `None` if there are no hunks after the cursor.
/// Wraps around to the first hunk if past the last.
#[must_use]
pub fn next_hunk_line(hunks: &[DiffHunk], cursor_line: usize) -> Option<usize> {
    if hunks.is_empty() {
        return None;
    }

    // Find first hunk starting after cursor (1-indexed in DiffHunk)
    let cursor_1indexed = cursor_line + 1;
    for hunk in hunks {
        if hunk.new_start > cursor_1indexed {
            return Some(hunk.new_start.saturating_sub(1));
        }
    }

    // Wrap around to first hunk
    Some(hunks[0].new_start.saturating_sub(1))
}

/// Find the index of the previous hunk before `cursor_line` (0-indexed).
///
/// Returns `None` if there are no hunks before the cursor.
/// Wraps around to the last hunk if before the first.
#[must_use]
pub fn prev_hunk_line(hunks: &[DiffHunk], cursor_line: usize) -> Option<usize> {
    if hunks.is_empty() {
        return None;
    }

    // Find last hunk starting before cursor (1-indexed in DiffHunk)
    let cursor_1indexed = cursor_line + 1;
    for hunk in hunks.iter().rev() {
        if hunk.new_start < cursor_1indexed {
            return Some(hunk.new_start.saturating_sub(1));
        }
    }

    // Wrap around to last hunk
    let last = &hunks[hunks.len() - 1];
    Some(last.new_start.saturating_sub(1))
}

/// Find the hunk that contains the given cursor line (0-indexed).
///
/// Returns `None` if the cursor is not inside any hunk.
#[must_use]
pub fn hunk_at_cursor(hunks: &[DiffHunk], cursor_line: usize) -> Option<&DiffHunk> {
    let cursor_1indexed = cursor_line + 1;
    hunks.iter().find(|h| {
        let start = h.new_start;
        let end = if h.new_count == 0 {
            // Deletion hunk — the marker is at new_start
            start + 1
        } else {
            start + h.new_count
        };
        cursor_1indexed >= start && cursor_1indexed < end
    })
}

/// Get file path from buffer for git operations.
///
/// Converts the path string to a `Path` reference.
#[must_use]
pub fn file_path_from_str(path: &str) -> &Path {
    Path::new(path)
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;
