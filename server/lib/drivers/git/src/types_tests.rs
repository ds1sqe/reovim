use {crate::types::*, std::path::PathBuf};

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
