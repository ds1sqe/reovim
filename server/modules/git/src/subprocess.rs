//! Subprocess-based git provider implementation.
//!
//! Shells out to the `git` CLI for all operations.
//! Returns empty results when `git` is not installed or
//! the path is not inside a repository.

use {
    reovim_driver_git::{
        GitProvider,
        types::{BranchInfo, DiffHunk, FileStatus, LogEntry, StashEntry, StatusEntry},
    },
    std::{
        path::{Path, PathBuf},
        process::Command,
    },
};

/// Git provider that uses the `git` subprocess.
///
/// All methods gracefully return empty results when `git` is not
/// available or the directory is not a git repository.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubprocessGitProvider;

impl SubprocessGitProvider {
    /// Create a new subprocess-based git provider.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl GitProvider for SubprocessGitProvider {
    fn current_branch(&self, cwd: &Path) -> Option<String> {
        let output = run_git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        let branch = output.trim().to_owned();
        if branch.is_empty() {
            None
        } else {
            Some(branch)
        }
    }

    fn branches(&self, cwd: &Path) -> Vec<BranchInfo> {
        let Some(output) =
            run_git(cwd, &["branch", "--list", "--format=%(HEAD)%(refname:short)\t%(upstream:short)"])
        else {
            return vec![];
        };

        output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let is_current = line.starts_with('*');
                let rest = if is_current { &line[1..] } else { line };

                let mut parts = rest.splitn(2, '\t');
                let name = parts.next().unwrap_or("").to_owned();
                let upstream = parts.next().and_then(|s| {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_owned())
                    }
                });

                BranchInfo {
                    name,
                    is_current,
                    upstream,
                }
            })
            .collect()
    }

    fn status(&self, cwd: &Path) -> Vec<StatusEntry> {
        let Some(output) = run_git(cwd, &["status", "--porcelain=v1"]) else {
            return vec![];
        };

        output
            .lines()
            .filter(|line| line.len() >= 4)
            .map(|line| {
                let bytes = line.as_bytes();
                let index_status = parse_file_status(bytes[0]);
                let worktree_status = parse_file_status(bytes[1]);
                let path = PathBuf::from(line[3..].trim());

                StatusEntry {
                    path,
                    index_status,
                    worktree_status,
                }
            })
            .collect()
    }

    fn log(&self, cwd: &Path, query: &str, limit: usize) -> Vec<LogEntry> {
        let limit_str = limit.to_string();
        let mut args = vec![
            "log",
            "--format=%H\t%h\t%aN\t%ai\t%s",
            "-n",
            &limit_str,
        ];
        if !query.is_empty() {
            args.push("--grep");
            args.push(query);
        }

        let Some(output) = run_git(cwd, &args) else {
            return vec![];
        };

        output
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let mut parts = line.splitn(5, '\t');
                Some(LogEntry {
                    hash: parts.next()?.to_owned(),
                    short_hash: parts.next()?.to_owned(),
                    author: parts.next()?.to_owned(),
                    date: parts.next()?.to_owned(),
                    message: parts.next()?.to_owned(),
                })
            })
            .collect()
    }

    fn stash_list(&self, cwd: &Path) -> Vec<StashEntry> {
        let Some(output) = run_git(cwd, &["stash", "list", "--format=%gd\t%gs"]) else {
            return vec![];
        };

        output
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let mut parts = line.splitn(2, '\t');
                let ref_name = parts.next()?;
                let message = parts.next().unwrap_or("").to_owned();

                // Parse "stash@{N}" to extract index N
                let index = ref_name
                    .strip_prefix("stash@{")
                    .and_then(|s| s.strip_suffix('}'))
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);

                Some(StashEntry { index, message })
            })
            .collect()
    }

    fn diff_hunks(&self, path: &Path) -> Vec<DiffHunk> {
        let path_str = path.to_string_lossy();
        let cwd = path.parent().unwrap_or_else(|| Path::new("."));

        let Some(output) = run_git(cwd, &["diff", "-U0", "--", &path_str]) else {
            return vec![];
        };

        output
            .lines()
            .filter(|line| line.starts_with("@@"))
            .filter_map(parse_hunk_header)
            .collect()
    }
}

/// Run a git command and return stdout on success.
fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

/// Parse a single-character porcelain v1 status code.
const fn parse_file_status(byte: u8) -> FileStatus {
    match byte {
        b'M' => FileStatus::Modified,
        b'A' => FileStatus::Added,
        b'D' => FileStatus::Deleted,
        b'R' => FileStatus::Renamed,
        b'C' => FileStatus::Copied,
        b'?' => FileStatus::Untracked,
        b'!' => FileStatus::Ignored,
        _ => FileStatus::Unmodified,
    }
}

/// Parse a diff hunk header like `@@ -10,3 +12,5 @@`.
fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
    // Format: @@ -old_start[,old_count] +new_start[,new_count] @@
    let trimmed = line.strip_prefix("@@ ")?;
    let end = trimmed.find(" @@")?;
    let range_part = &trimmed[..end];

    let mut parts = range_part.split(' ');
    let old_range = parts.next()?.strip_prefix('-')?;
    let new_range = parts.next()?.strip_prefix('+')?;

    let (old_start, old_count) = parse_range(old_range);
    let (new_start, new_count) = parse_range(new_range);

    Some(DiffHunk {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

/// Parse a range like "10,3" or "10" (count defaults to 1).
fn parse_range(s: &str) -> (usize, usize) {
    if let Some((start, count)) = s.split_once(',') {
        (
            start.parse().unwrap_or(0),
            count.parse().unwrap_or(1),
        )
    } else {
        (s.parse().unwrap_or(0), 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_file_status_all_codes() {
        assert_eq!(parse_file_status(b'M'), FileStatus::Modified);
        assert_eq!(parse_file_status(b'A'), FileStatus::Added);
        assert_eq!(parse_file_status(b'D'), FileStatus::Deleted);
        assert_eq!(parse_file_status(b'R'), FileStatus::Renamed);
        assert_eq!(parse_file_status(b'C'), FileStatus::Copied);
        assert_eq!(parse_file_status(b'?'), FileStatus::Untracked);
        assert_eq!(parse_file_status(b'!'), FileStatus::Ignored);
        assert_eq!(parse_file_status(b' '), FileStatus::Unmodified);
        assert_eq!(parse_file_status(b'X'), FileStatus::Unmodified);
    }

    #[test]
    fn parse_hunk_header_standard() {
        let hunk = parse_hunk_header("@@ -10,3 +12,5 @@ fn main()").unwrap();
        assert_eq!(hunk.old_start, 10);
        assert_eq!(hunk.old_count, 3);
        assert_eq!(hunk.new_start, 12);
        assert_eq!(hunk.new_count, 5);
    }

    #[test]
    fn parse_hunk_header_single_line() {
        let hunk = parse_hunk_header("@@ -1 +1 @@").unwrap();
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_count, 1);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_count, 1);
    }

    #[test]
    fn parse_hunk_header_zero_count() {
        let hunk = parse_hunk_header("@@ -5,0 +6,2 @@").unwrap();
        assert_eq!(hunk.old_start, 5);
        assert_eq!(hunk.old_count, 0);
        assert_eq!(hunk.new_start, 6);
        assert_eq!(hunk.new_count, 2);
    }

    #[test]
    fn parse_hunk_header_invalid() {
        assert!(parse_hunk_header("not a hunk").is_none());
        assert!(parse_hunk_header("@@").is_none());
        assert!(parse_hunk_header("@@ @@").is_none());
    }

    #[test]
    fn parse_range_with_count() {
        assert_eq!(parse_range("10,3"), (10, 3));
    }

    #[test]
    fn parse_range_without_count() {
        assert_eq!(parse_range("42"), (42, 1));
    }

    #[test]
    fn parse_range_invalid() {
        assert_eq!(parse_range("abc"), (0, 1));
        assert_eq!(parse_range("abc,xyz"), (0, 1));
    }

    #[test]
    fn subprocess_provider_debug_clone_default() {
        let provider = SubprocessGitProvider::new();
        let cloned = provider;
        let debug = format!("{cloned:?}");
        assert!(debug.contains("SubprocessGitProvider"));

        #[allow(clippy::default_constructed_unit_structs)]
        let _ = SubprocessGitProvider::default();
    }

    #[test]
    fn run_git_nonexistent_dir() {
        // Should return None for a non-existent directory
        let result = run_git(Path::new("/nonexistent/path/that/does/not/exist"), &["status"]);
        assert!(result.is_none());
    }

    #[test]
    fn run_git_invalid_command() {
        // Should return None for an invalid git command
        let result = run_git(Path::new("."), &["not-a-real-command"]);
        assert!(result.is_none());
    }
}
