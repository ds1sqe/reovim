use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use {
    reovim_driver_display::annotation::{AnnotationContext, AnnotationSource},
    reovim_driver_git::{
        GitProvider,
        types::{BlameEntry, BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry},
    },
    reovim_kernel::api::v1::BufferId,
};

use super::*;

struct MockGitProvider {
    blame_entries: Vec<BlameEntry>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl GitProvider for MockGitProvider {
    fn current_branch(&self, _cwd: &Path) -> Option<String> {
        None
    }
    fn branches(&self, _cwd: &Path) -> Vec<BranchInfo> {
        vec![]
    }
    fn status(&self, _cwd: &Path) -> Vec<StatusEntry> {
        vec![]
    }
    fn log(&self, _cwd: &Path, _query: &str, _limit: usize) -> Vec<LogEntry> {
        vec![]
    }
    fn stash_list(&self, _cwd: &Path) -> Vec<StashEntry> {
        vec![]
    }
    fn diff_hunks(&self, _path: &Path) -> Vec<DiffHunk> {
        vec![]
    }
    fn blame(&self, _path: &Path) -> Vec<BlameEntry> {
        self.blame_entries.clone()
    }
}

fn sample_entries() -> Vec<BlameEntry> {
    vec![
        BlameEntry {
            line: 1,
            short_hash: "abc1234".to_owned(),
            author: "Alice".to_owned(),
            date: "2025-01-15".to_owned(),
            summary: "feat: first".to_owned(),
        },
        BlameEntry {
            line: 2,
            short_hash: "def5678".to_owned(),
            author: "Bob".to_owned(),
            date: "2025-01-16".to_owned(),
            summary: "fix: second".to_owned(),
        },
        BlameEntry {
            line: 3,
            short_hash: "ghi9012".to_owned(),
            author: "Carol".to_owned(),
            date: "2025-01-17".to_owned(),
            summary: "chore: third".to_owned(),
        },
    ]
}

fn make_source(entries: Vec<BlameEntry>) -> BlameAnnotationSource {
    let provider: Arc<dyn GitProvider> = Arc::new(MockGitProvider {
        blame_entries: entries,
    });
    BlameAnnotationSource::new(provider)
}

fn context_with_path(path: &str) -> AnnotationContext {
    let mut ctx = AnnotationContext::minimal(100);
    ctx.file_path = Some(PathBuf::from(path));
    ctx
}

#[test]
fn test_id() {
    let source = make_source(vec![]);
    assert_eq!(source.id(), "plugin.git-blame");
}

#[test]
fn test_provides() {
    let source = make_source(vec![]);
    let kinds = source.provides();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0].name(), "blame.info");
}

#[test]
fn test_annotations_not_active() {
    let source = make_source(sample_entries());
    let ctx = context_with_path("/tmp/test.rs");
    let result = source.annotations(BufferId::new(), 0..100, &ctx);
    assert!(result.is_empty());
}

#[test]
fn test_annotations_active_with_cache() {
    let source = make_source(sample_entries());
    let buf = BufferId::new();
    let path = PathBuf::from("/tmp/test.rs");
    source.toggle(buf, Some(&path));

    let ctx = context_with_path("/tmp/test.rs");
    let result = source.annotations(buf, 0..100, &ctx);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].kind.name(), "blame.info");
    assert!(result[0].payload.as_text().unwrap().contains("Alice"));
}

#[test]
fn test_annotations_active_no_file_path() {
    let source = make_source(sample_entries());
    let buf = BufferId::new();
    let path = PathBuf::from("/tmp/test.rs");
    source.toggle(buf, Some(&path));

    let ctx = AnnotationContext::minimal(100);
    let result = source.annotations(buf, 0..100, &ctx);
    assert!(result.is_empty());
}

#[test]
fn test_annotations_active_uncached_path() {
    let source = make_source(sample_entries());
    let buf = BufferId::new();
    // Toggle with one path
    let path = PathBuf::from("/tmp/other.rs");
    source.toggle(buf, Some(&path));

    // Query with a different path — triggers fetch
    let ctx = context_with_path("/tmp/test.rs");
    let result = source.annotations(buf, 0..100, &ctx);
    assert_eq!(result.len(), 3);
}

#[test]
fn test_annotations_range_filtering() {
    let source = make_source(sample_entries());
    let buf = BufferId::new();
    let path = PathBuf::from("/tmp/test.rs");
    source.toggle(buf, Some(&path));

    let ctx = context_with_path("/tmp/test.rs");
    // Only line 0 (blame line 1 → index 0)
    let result = source.annotations(buf, 0..1, &ctx);
    assert_eq!(result.len(), 1);
    assert!(result[0].payload.as_text().unwrap().contains("Alice"));
}

#[test]
fn test_toggle_on_off() {
    let source = make_source(vec![]);
    let buf = BufferId::new();
    assert!(!source.is_active(buf));

    source.toggle(buf, None);
    assert!(source.is_active(buf));

    source.toggle(buf, None);
    assert!(!source.is_active(buf));
}

#[test]
fn test_annotation_priority() {
    let source = make_source(sample_entries());
    let buf = BufferId::new();
    let path = PathBuf::from("/tmp/test.rs");
    source.toggle(buf, Some(&path));

    let ctx = context_with_path("/tmp/test.rs");
    let result = source.annotations(buf, 0..1, &ctx);
    assert_eq!(result[0].priority, 5);
}
