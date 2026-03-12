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
