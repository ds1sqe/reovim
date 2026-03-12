use super::subprocess::*;

use {
    reovim_driver_git::{GitProvider, types::FileStatus},
    std::{
        path::{Path, PathBuf},
        process::Command,
    },
};

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

// -- Integration tests using the real repo --
// These tests run against the actual git repository we're in.

/// Helper to get the repo root.
fn repo_root() -> PathBuf {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git available");
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}

#[test]
fn current_branch_in_repo() {
    let provider = SubprocessGitProvider::new();
    let root = repo_root();
    let branch = provider.current_branch(&root);
    // We're in a git repo, so this should return something
    assert!(branch.is_some());
    assert!(!branch.unwrap().is_empty());
}

#[test]
fn current_branch_empty_on_nonrepo() {
    let provider = SubprocessGitProvider::new();
    let result = provider.current_branch(Path::new("/tmp"));
    // /tmp is not a git repo — returns None
    assert!(result.is_none());
}

#[test]
fn branches_in_repo() {
    let provider = SubprocessGitProvider::new();
    let root = repo_root();
    let branches = provider.branches(&root);
    // A real repo has at least one branch
    assert!(!branches.is_empty());
    // Exactly one branch should be current
    let current_count = branches.iter().filter(|b| b.is_current).count();
    assert_eq!(current_count, 1);
    // All branch names are non-empty
    assert!(branches.iter().all(|b| !b.name.is_empty()));
}

#[test]
fn branches_empty_on_nonrepo() {
    let provider = SubprocessGitProvider::new();
    assert!(provider.branches(Path::new("/tmp")).is_empty());
}

#[test]
fn status_in_repo() {
    let provider = SubprocessGitProvider::new();
    let root = repo_root();
    // Status should not panic — may be empty if working tree is clean
    let _ = provider.status(&root);
}

#[test]
fn status_empty_on_nonrepo() {
    let provider = SubprocessGitProvider::new();
    assert!(provider.status(Path::new("/tmp")).is_empty());
}

#[test]
fn log_in_repo() {
    let provider = SubprocessGitProvider::new();
    let root = repo_root();
    let entries = provider.log(&root, "", 5);
    // A real repo with commits should return entries
    assert!(!entries.is_empty());
    assert!(entries.len() <= 5);
    // Each entry should have non-empty fields
    for entry in &entries {
        assert!(!entry.hash.is_empty());
        assert!(!entry.short_hash.is_empty());
        assert!(!entry.author.is_empty());
        assert!(!entry.date.is_empty());
    }
}

#[test]
fn log_with_query() {
    let provider = SubprocessGitProvider::new();
    let root = repo_root();
    // Query for something that likely exists
    let entries = provider.log(&root, "feat", 10);
    // May be empty if no commit message contains "feat", but shouldn't panic
    for entry in &entries {
        assert!(!entry.hash.is_empty());
    }
}

#[test]
fn log_empty_on_nonrepo() {
    let provider = SubprocessGitProvider::new();
    assert!(provider.log(Path::new("/tmp"), "", 10).is_empty());
}

#[test]
fn stash_list_in_repo() {
    let provider = SubprocessGitProvider::new();
    let root = repo_root();
    // Stash may be empty but should not panic
    let _ = provider.stash_list(&root);
}

#[test]
fn stash_list_empty_on_nonrepo() {
    let provider = SubprocessGitProvider::new();
    assert!(provider.stash_list(Path::new("/tmp")).is_empty());
}

#[test]
fn diff_hunks_in_repo() {
    let provider = SubprocessGitProvider::new();
    let root = repo_root();
    // Diff on a known file — may return empty if no changes
    let path = root.join("Cargo.toml");
    let _ = provider.diff_hunks(&path);
}

#[test]
fn diff_hunks_nonexistent_file() {
    let provider = SubprocessGitProvider::new();
    let hunks = provider.diff_hunks(Path::new("/nonexistent/file.rs"));
    assert!(hunks.is_empty());
}

#[test]
fn run_git_success() {
    let root = repo_root();
    let result = run_git(&root, &["rev-parse", "--git-dir"]);
    assert!(result.is_some());
    assert!(result.unwrap().contains(".git"));
}

#[test]
fn run_git_failed_command() {
    let root = repo_root();
    // git log on a non-existent ref should fail
    let result = run_git(&root, &["log", "--format=%H", "nonexistent-ref-abc123"]);
    assert!(result.is_none());
}

#[test]
fn parse_branch_name_nonempty() {
    assert_eq!(parse_branch_name("main\n"), Some("main".to_owned()));
    assert_eq!(parse_branch_name("  feature/foo  \n"), Some("feature/foo".to_owned()));
}

#[test]
fn parse_branch_name_empty() {
    assert!(parse_branch_name("").is_none());
    assert!(parse_branch_name("  \n").is_none());
}

// ===== Blame parsing tests =====

#[test]
fn parse_porcelain_blame_single_entry() {
    let output = "\
abc1234567890abcdef1234567890abcdef12345678 1 1 1\n\
author Alice\n\
author-mail <alice@example.com>\n\
author-time 1700000000\n\
author-tz +0000\n\
committer Alice\n\
committer-mail <alice@example.com>\n\
committer-time 1700000000\n\
committer-tz +0000\n\
summary feat: initial commit\n\
filename src/main.rs\n\
\tlet x = 42;\n";
    let entries = parse_porcelain_blame(output);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].line, 1);
    assert_eq!(entries[0].short_hash, "abc1234");
    assert_eq!(entries[0].author, "Alice");
    assert_eq!(entries[0].date, "1700000000");
    assert_eq!(entries[0].summary, "feat: initial commit");
}

#[test]
fn parse_porcelain_blame_multiple_entries() {
    let output = "\
aaaaaaa000000000000000000000000000000000000 1 1 1\n\
author Alice\n\
author-time 1700000000\n\
summary first line\n\
filename f.rs\n\
\tline one\n\
bbbbbbb111111111111111111111111111111111111 1 2 1\n\
author Bob\n\
author-time 1700001000\n\
summary second line\n\
filename f.rs\n\
\tline two\n";
    let entries = parse_porcelain_blame(output);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].author, "Alice");
    assert_eq!(entries[0].line, 1);
    assert_eq!(entries[1].author, "Bob");
    assert_eq!(entries[1].line, 2);
}

#[test]
fn parse_porcelain_blame_empty_output() {
    let entries = parse_porcelain_blame("");
    assert!(entries.is_empty());
}

#[test]
fn blame_nonexistent_file() {
    let provider = SubprocessGitProvider::new();
    let result = provider.blame(Path::new("/nonexistent/path/file.rs"));
    assert!(result.is_empty());
}
