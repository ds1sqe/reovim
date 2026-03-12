use std::{path::Path, sync::Arc};

use {
    reovim_driver_display::statusline::{ComponentContext, ComponentProvider},
    reovim_driver_git::{
        GitProvider,
        types::{BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry},
    },
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

fn context_with_filepath(path: &str) -> ComponentContext {
    ComponentContext {
        filepath: Some(path.to_string()),
        ..ComponentContext::default()
    }
}

#[test]
fn test_id() {
    let comp = BranchComponent::new(mock_provider(Some("main")));
    assert_eq!(comp.id(), "branch");
}

#[test]
fn test_render_with_branch() {
    let comp = BranchComponent::new(mock_provider(Some("main")));
    let ctx = context_with_filepath("/home/user/project/src/lib.rs");
    let output = comp.render(&ctx);
    assert!(output.visible);
    assert_eq!(output.text, " main ");
}

#[test]
fn test_render_no_filepath() {
    let comp = BranchComponent::new(mock_provider(Some("main")));
    let ctx = ComponentContext::default();
    let output = comp.render(&ctx);
    assert!(!output.visible);
}

#[test]
fn test_render_no_branch() {
    let comp = BranchComponent::new(mock_provider(None));
    let ctx = context_with_filepath("/home/user/project/src/lib.rs");
    let output = comp.render(&ctx);
    assert!(!output.visible);
}

#[test]
fn test_render_filepath_no_parent() {
    let comp = BranchComponent::new(mock_provider(Some("main")));
    let ctx = context_with_filepath("file.rs");
    let output = comp.render(&ctx);
    // "file.rs" has parent "" which is empty, so hidden
    assert!(!output.visible);
}

#[test]
fn test_render_feature_branch() {
    let comp = BranchComponent::new(mock_provider(Some("feature/git-provider")));
    let ctx = context_with_filepath("/tmp/src/main.rs");
    let output = comp.render(&ctx);
    assert!(output.visible);
    assert_eq!(output.text, " feature/git-provider ");
}
