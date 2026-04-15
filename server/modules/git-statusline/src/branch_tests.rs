use std::{path::Path, sync::Arc};

use {
    reovim_driver_git::{
        GitProvider,
        types::{BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry},
    },
    reovim_driver_statusline::{ComponentDataContext, ComponentDataProvider},
};

use super::*;

struct MockGitProvider {
    branch: Option<String>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl GitProvider for MockGitProvider {
    fn current_branch(&self, _cwd: &Path) -> Option<String> {
        self.branch.clone()
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
}

fn mock_provider(branch: Option<&str>) -> Arc<dyn GitProvider> {
    Arc::new(MockGitProvider {
        branch: branch.map(String::from),
    })
}

fn context_with_filepath(path: &str) -> ComponentDataContext {
    ComponentDataContext {
        filepath: Some(path.to_string()),
        ..ComponentDataContext::default()
    }
}

#[test]
fn test_id() {
    let comp = BranchComponent::new(mock_provider(Some("main")));
    assert_eq!(comp.id(), "branch");
}

#[test]
fn test_data_with_branch() {
    let comp = BranchComponent::new(mock_provider(Some("main")));
    let ctx = context_with_filepath("/home/user/project/src/lib.rs");
    let data = comp.data(&ctx);
    assert!(data.visible);
    assert_eq!(data.text, " main ");
}

#[test]
fn test_data_no_filepath() {
    let comp = BranchComponent::new(mock_provider(Some("main")));
    let ctx = ComponentDataContext::default();
    let data = comp.data(&ctx);
    assert!(!data.visible);
}

#[test]
fn test_data_no_branch() {
    let comp = BranchComponent::new(mock_provider(None));
    let ctx = context_with_filepath("/home/user/project/src/lib.rs");
    let data = comp.data(&ctx);
    assert!(!data.visible);
}

#[test]
fn test_data_filepath_no_parent() {
    let comp = BranchComponent::new(mock_provider(Some("main")));
    let ctx = context_with_filepath("file.rs");
    let data = comp.data(&ctx);
    // "file.rs" has parent "" which is empty, so hidden
    assert!(!data.visible);
}

#[test]
fn test_data_feature_branch() {
    let comp = BranchComponent::new(mock_provider(Some("feature/git-provider")));
    let ctx = context_with_filepath("/tmp/src/main.rs");
    let data = comp.data(&ctx);
    assert!(data.visible);
    assert_eq!(data.text, " feature/git-provider ");
}
