//! Typed data structures for git operations.
//!
//! These replace stringly-typed `PickerData::Text(String)` with
//! semantically meaningful types that the compiler can check.

use std::path::PathBuf;

/// Information about a git branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchInfo {
    /// Branch name (e.g., "main", "feature/foo").
    pub name: String,
    /// Whether this is the currently checked-out branch.
    pub is_current: bool,
    /// Upstream tracking branch, if any (e.g., "origin/main").
    pub upstream: Option<String>,
}

/// A file's status in the git index or working tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// No changes.
    Unmodified,
    /// Content modified.
    Modified,
    /// Newly added to index.
    Added,
    /// Deleted.
    Deleted,
    /// Renamed.
    Renamed,
    /// Copied.
    Copied,
    /// Not tracked by git.
    Untracked,
    /// Ignored by `.gitignore`.
    Ignored,
}

/// A file's combined index + working tree status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// Path relative to the repository root.
    pub path: PathBuf,
    /// Status in the index (staging area).
    pub index_status: FileStatus,
    /// Status in the working tree.
    pub worktree_status: FileStatus,
}

/// A single commit log entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// Full commit hash.
    pub hash: String,
    /// Abbreviated commit hash.
    pub short_hash: String,
    /// Author name.
    pub author: String,
    /// Commit date (ISO 8601).
    pub date: String,
    /// First line of the commit message.
    pub message: String,
}

/// A stash entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    /// Stash index (0 = most recent).
    pub index: usize,
    /// Stash message / description.
    pub message: String,
}

/// A single diff hunk header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffHunk {
    /// Starting line in the old file.
    pub old_start: usize,
    /// Number of lines removed.
    pub old_count: usize,
    /// Starting line in the new file.
    pub new_start: usize,
    /// Number of lines added.
    pub new_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_info_debug_clone_eq() {
        let branch = BranchInfo {
            name: "main".to_owned(),
            is_current: true,
            upstream: Some("origin/main".to_owned()),
        };
        let cloned = branch.clone();
        assert_eq!(branch, cloned);
        let debug = format!("{branch:?}");
        assert!(debug.contains("main"));
    }

    #[test]
    fn file_status_copy_eq() {
        let status = FileStatus::Modified;
        let copied = status;
        assert_eq!(status, copied);
        assert_ne!(FileStatus::Added, FileStatus::Deleted);
    }

    #[test]
    fn file_status_all_variants() {
        let variants = [
            FileStatus::Unmodified,
            FileStatus::Modified,
            FileStatus::Added,
            FileStatus::Deleted,
            FileStatus::Renamed,
            FileStatus::Copied,
            FileStatus::Untracked,
            FileStatus::Ignored,
        ];
        // All variants are distinct
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                assert_eq!(i == j, a == b);
            }
        }
    }

    #[test]
    fn status_entry_debug_clone_eq() {
        let entry = StatusEntry {
            path: PathBuf::from("src/main.rs"),
            index_status: FileStatus::Modified,
            worktree_status: FileStatus::Unmodified,
        };
        let cloned = entry.clone();
        assert_eq!(entry, cloned);
        let debug = format!("{entry:?}");
        assert!(debug.contains("main.rs"));
    }

    #[test]
    fn log_entry_debug_clone_eq() {
        let entry = LogEntry {
            hash: "abc123def456".to_owned(),
            short_hash: "abc123d".to_owned(),
            author: "Alice".to_owned(),
            date: "2025-01-01".to_owned(),
            message: "feat: add feature".to_owned(),
        };
        let cloned = entry.clone();
        assert_eq!(entry, cloned);
        let debug = format!("{entry:?}");
        assert!(debug.contains("Alice"));
    }

    #[test]
    fn stash_entry_debug_clone_eq() {
        let entry = StashEntry {
            index: 0,
            message: "WIP on main".to_owned(),
        };
        let cloned = entry.clone();
        assert_eq!(entry, cloned);
        let debug = format!("{entry:?}");
        assert!(debug.contains("WIP"));
    }

    #[test]
    fn diff_hunk_debug_copy_eq() {
        let hunk = DiffHunk {
            old_start: 10,
            old_count: 3,
            new_start: 10,
            new_count: 5,
        };
        let copied = hunk;
        assert_eq!(hunk, copied);
        let debug = format!("{hunk:?}");
        assert!(debug.contains("10"));
    }
}
