//! Subprocess-based git provider implementation.
//!
//! Shells out to the `git` CLI for all operations.
//! Returns empty results when `git` is not installed or
//! the path is not inside a repository.

use {
    reovim_driver_git::{
        GitProvider,
        types::{BlameEntry, BranchInfo, DiffHunk, FileStatus, LogEntry, StashEntry, StatusEntry},
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

// coverage(off): all methods delegate to run_git (subprocess execution)
#[cfg_attr(coverage_nightly, coverage(off))]
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

    fn blame(&self, path: &Path) -> Vec<BlameEntry> {
        let path_str = path.to_string_lossy();
        let cwd = path.parent().unwrap_or_else(|| Path::new("."));

        let Some(output) = run_git(cwd, &["blame", "--porcelain", &path_str]) else {
            return vec![];
        };

        parse_porcelain_blame(&output)
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

    fn stage_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let cwd = path.parent().unwrap_or_else(|| Path::new("."));
        run_git(cwd, &["add", "--", &path_str]).is_some()
    }

    fn reset_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let cwd = path.parent().unwrap_or_else(|| Path::new("."));
        run_git(cwd, &["checkout", "--", &path_str]).is_some()
    }

    fn unstage_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let cwd = path.parent().unwrap_or_else(|| Path::new("."));
        run_git(cwd, &["reset", "HEAD", "--", &path_str]).is_some()
    }

    fn stage_lines(&self, cwd: &Path, patch: &str) -> bool {
        run_git_with_stdin(cwd, &["apply", "--cached"], patch)
    }

    fn reset_lines(&self, cwd: &Path, patch: &str) -> bool {
        run_git_with_stdin(cwd, &["apply", "--reverse"], patch)
    }

    fn diff_content(&self, path: &Path) -> Option<String> {
        let path_str = path.to_string_lossy();
        let cwd = path.parent().unwrap_or_else(|| Path::new("."));
        run_git(cwd, &["diff", "--", &path_str])
    }
}

/// Parse the output of `git rev-parse --abbrev-ref HEAD`.
/// Returns `None` if the output is empty (e.g. bare repo).
pub(crate) fn parse_branch_name(output: &str) -> Option<String> {
    let branch = output.trim().to_owned();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

/// Run a git command and return stdout on success.
// coverage(off): spawns std::process::Command — requires `git` binary
#[cfg_attr(coverage_nightly, coverage(off))]
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

/// Run a git command with stdin input, returning true on success.
// coverage(off): spawns std::process::Command with piped stdin
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) fn run_git_with_stdin(cwd: &Path, args: &[&str], stdin_data: &str) -> bool {
    use std::{io::Write as _, process::Stdio};

    let Ok(mut child) = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };

    if let Some(ref mut stdin) = child.stdin {
        let _ = stdin.write_all(stdin_data.as_bytes());
    }

    child.wait().is_ok_and(|s| s.success())
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

/// Parse `git blame --porcelain` output into `BlameEntry` list.
///
/// Porcelain format groups: header line (hash, orig line, final line, count),
/// followed by key-value metadata lines, then a tab-prefixed content line.
pub(crate) fn parse_porcelain_blame(output: &str) -> Vec<BlameEntry> {
    let mut entries = Vec::new();
    let mut current_hash = String::new();
    let mut current_line: usize = 0;
    let mut author = String::new();
    let mut date = String::new();
    let mut summary = String::new();

    for line in output.lines() {
        if line.starts_with('\t') {
            // Content line — marks end of this entry
            let short_hash = if current_hash.len() >= 7 {
                current_hash[..7].to_owned()
            } else {
                current_hash.clone()
            };
            entries.push(BlameEntry {
                line: current_line,
                short_hash,
                author: std::mem::take(&mut author),
                date: std::mem::take(&mut date),
                summary: std::mem::take(&mut summary),
            });
        } else if let Some(rest) = line.strip_prefix("author ") {
            rest.clone_into(&mut author);
        } else if let Some(rest) = line.strip_prefix("author-time ") {
            rest.clone_into(&mut date);
        } else if let Some(rest) = line.strip_prefix("summary ") {
            rest.clone_into(&mut summary);
        } else {
            // Try parsing as header: <hash> <orig-line> <final-line> [<count>]
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3
                && parts[0].len() >= 7
                && parts[0].chars().all(|c| c.is_ascii_hexdigit())
            {
                parts[0].clone_into(&mut current_hash);
                current_line = parts[2].parse().unwrap_or(0);
            }
        }
    }

    entries
}

/// Parse a range like "10,3" or "10" (count defaults to 1).
pub(crate) fn parse_range(s: &str) -> (usize, usize) {
    if let Some((start, count)) = s.split_once(',') {
        (start.parse().unwrap_or(0), count.parse().unwrap_or(1))
    } else {
        (s.parse().unwrap_or(0), 1)
    }
}
