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
        parse_branch_name(&output)
    }

    fn branches(&self, cwd: &Path) -> Vec<BranchInfo> {
        let Some(output) = run_git(
            cwd,
            &[
                "branch",
                "--list",
                "--format=%(HEAD)%(refname:short)\t%(upstream:short)",
            ],
        ) else {
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
        let mut args = vec!["log", "--format=%H\t%h\t%aN\t%ai\t%s", "-n", &limit_str];
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

/// Parse the output of `git rev-parse --abbrev-ref HEAD`.
/// Returns `None` if the output is empty (e.g. bare repo).
pub(crate) fn parse_branch_name(output: &str) -> Option<String> {
    let branch = output.trim().to_owned();
    if branch.is_empty() { None } else { Some(branch) }
}

/// Run a git command and return stdout on success.
pub(crate) fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
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
pub(crate) const fn parse_file_status(byte: u8) -> FileStatus {
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
pub(crate) fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
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
pub(crate) fn parse_range(s: &str) -> (usize, usize) {
    if let Some((start, count)) = s.split_once(',') {
        (start.parse().unwrap_or(0), count.parse().unwrap_or(1))
    } else {
        (s.parse().unwrap_or(0), 1)
    }
}
