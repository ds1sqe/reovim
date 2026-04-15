use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use {
    reovim_kernel::api::v1::{BufferId, ServiceRegistry},
    reovim_driver_annotation::{AnnotationContext, AnnotationSource},
    reovim_driver_git::{
        GitProvider, GitProviderStore,
        types::{BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry},
    },
};

use super::*;

struct MockGitProvider {
    hunks: Vec<DiffHunk>,
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
        self.hunks.clone()
    }
}

fn setup_services(hunks: Vec<DiffHunk>) -> Arc<ServiceRegistry> {
    let services = Arc::new(ServiceRegistry::new());
    let store = services.get_or_create::<GitProviderStore>();
    store.register(Arc::new(MockGitProvider { hunks }));
    services
}

fn context_with_path(path: &str) -> AnnotationContext {
    let mut ctx = AnnotationContext::minimal(100);
    ctx.file_path = Some(PathBuf::from(path));
    ctx
}

#[test]
fn test_id() {
    let services = Arc::new(ServiceRegistry::new());
    let source = GitSignsSource::new(services);
    assert_eq!(source.id(), "plugin.git-signs");
}

#[test]
fn test_provides_three_kinds() {
    let services = Arc::new(ServiceRegistry::new());
    let source = GitSignsSource::new(services);
    let kinds = source.provides();
    assert_eq!(kinds.len(), 3);
    assert_eq!(kinds[0].name(), "git.add");
    assert_eq!(kinds[1].name(), "git.change");
    assert_eq!(kinds[2].name(), "git.delete");
}

#[test]
fn test_annotations_no_file_path() {
    let services = setup_services(vec![DiffHunk {
        old_start: 1,
        old_count: 0,
        new_start: 1,
        new_count: 3,
    }]);
    let source = GitSignsSource::new(services);
    let ctx = AnnotationContext::minimal(100);
    let result = source.annotations(BufferId::new(), 0..100, &ctx);
    assert!(result.is_empty());
}

#[test]
fn test_annotations_no_store() {
    let services = Arc::new(ServiceRegistry::new());
    let source = GitSignsSource::new(services);
    let ctx = context_with_path("/tmp/test.rs");
    let result = source.annotations(BufferId::new(), 0..100, &ctx);
    assert!(result.is_empty());
}

#[test]
fn test_annotations_no_provider() {
    let services = Arc::new(ServiceRegistry::new());
    let _store = services.get_or_create::<GitProviderStore>();
    let source = GitSignsSource::new(services);
    let ctx = context_with_path("/tmp/test.rs");
    let result = source.annotations(BufferId::new(), 0..100, &ctx);
    assert!(result.is_empty());
}

#[test]
fn test_annotations_with_hunks() {
    let services = setup_services(vec![
        DiffHunk {
            old_start: 5,
            old_count: 0,
            new_start: 5,
            new_count: 2,
        },
        DiffHunk {
            old_start: 10,
            old_count: 3,
            new_start: 12,
            new_count: 0,
        },
        DiffHunk {
            old_start: 20,
            old_count: 1,
            new_start: 19,
            new_count: 2,
        },
    ]);
    let source = GitSignsSource::new(services);
    let ctx = context_with_path("/tmp/test.rs");
    let result = source.annotations(BufferId::new(), 0..100, &ctx);

    // 2 adds + 1 delete + 2 changes = 5 annotations
    assert_eq!(result.len(), 5);
    assert_eq!(result[0].kind.name(), "git.add");
    assert_eq!(result[1].kind.name(), "git.add");
    assert_eq!(result[2].kind.name(), "git.delete");
    assert_eq!(result[3].kind.name(), "git.change");
    assert_eq!(result[4].kind.name(), "git.change");
}

#[test]
fn test_annotations_range_filtering() {
    let services = setup_services(vec![
        DiffHunk {
            old_start: 3,
            old_count: 0,
            new_start: 3,
            new_count: 2,
        },
        DiffHunk {
            old_start: 50,
            old_count: 0,
            new_start: 50,
            new_count: 1,
        },
    ]);
    let source = GitSignsSource::new(services);
    let ctx = context_with_path("/tmp/test.rs");

    // Only query lines 0..10 — should get lines 2,3 (from hunk new_start=3)
    let result = source.annotations(BufferId::new(), 0..10, &ctx);
    assert_eq!(result.len(), 2);

    // Only query lines 40..60 — should get line 49 (from hunk new_start=50)
    let result = source.annotations(BufferId::new(), 40..60, &ctx);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_annotations_empty_hunks() {
    let services = setup_services(vec![]);
    let source = GitSignsSource::new(services);
    let ctx = context_with_path("/tmp/test.rs");
    let result = source.annotations(BufferId::new(), 0..100, &ctx);
    assert!(result.is_empty());
}

#[test]
fn test_annotation_priority() {
    let services = setup_services(vec![DiffHunk {
        old_start: 1,
        old_count: 0,
        new_start: 1,
        new_count: 1,
    }]);
    let source = GitSignsSource::new(services);
    let ctx = context_with_path("/tmp/test.rs");
    let result = source.annotations(BufferId::new(), 0..10, &ctx);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].priority, 10);
}

#[test]
fn test_kind_str_coverage() {
    assert_eq!(GitSignsSource::kind_str(SignKind::Add), "git.add");
    assert_eq!(GitSignsSource::kind_str(SignKind::Change), "git.change");
    assert_eq!(GitSignsSource::kind_str(SignKind::Delete), "git.delete");
}
