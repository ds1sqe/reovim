use {
    super::*,
    reovim_kernel::api::v1::{Module, ModuleContext, ProbeResult},
    reovim_subsys_annotation::AnnotationSourceRegistry,
    reovim_subsys_git::{GitProvider, GitProviderStore},
    std::sync::Arc,
};

#[test]
fn test_module_id() {
    let module = GitBlameModule::new();
    assert_eq!(module.id().as_str(), "git-blame");
}

#[test]
fn test_module_name() {
    let module = GitBlameModule::new();
    assert_eq!(module.name(), "Git Blame");
}

#[test]
fn test_module_version() {
    let module = GitBlameModule::new();
    let v = module.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn test_module_default() {
    let m1 = GitBlameModule::new();
    let m2 = GitBlameModule;
    assert_eq!(m1.id(), m2.id());
}

#[test]
fn test_exit_succeeds() {
    let mut module = GitBlameModule::new();
    assert!(module.exit().is_ok());
}

// ========================================================================
// init() coverage
// ========================================================================

struct StubGitProvider;
impl GitProvider for StubGitProvider {
    fn current_branch(&self, _: &std::path::Path) -> Option<String> {
        None
    }
    fn branches(&self, _: &std::path::Path) -> Vec<reovim_subsys_git::types::BranchInfo> {
        vec![]
    }
    fn status(&self, _: &std::path::Path) -> Vec<reovim_subsys_git::types::StatusEntry> {
        vec![]
    }
    fn log(
        &self,
        _: &std::path::Path,
        _: &str,
        _: usize,
    ) -> Vec<reovim_subsys_git::types::LogEntry> {
        vec![]
    }
    fn stash_list(&self, _: &std::path::Path) -> Vec<reovim_subsys_git::types::StashEntry> {
        vec![]
    }
    fn diff_hunks(&self, _: &std::path::Path) -> Vec<reovim_subsys_git::types::DiffHunk> {
        vec![]
    }
}

/// No git provider → nothing registered, returns Success.
#[test]
fn test_init_no_git_provider() {
    let mut module = GitBlameModule::new();
    let ctx = ModuleContext::default();
    assert_eq!(module.init(&ctx), ProbeResult::Success);

    // No source should be registered without a git provider
    let registry = ctx.services.get::<AnnotationSourceRegistry>();
    assert!(registry.is_none() || registry.unwrap().is_empty(), "no source without provider");
}

/// Git provider exists → registers source in `AnnotationSourceRegistry`.
#[test]
fn test_init_with_git_provider() {
    let mut module = GitBlameModule::new();
    let ctx = ModuleContext::default();

    let store = ctx.services.get_or_create::<GitProviderStore>();
    store.register(Arc::new(StubGitProvider));

    assert_eq!(module.init(&ctx), ProbeResult::Success);

    let registry = ctx
        .services
        .get::<AnnotationSourceRegistry>()
        .expect("AnnotationSourceRegistry should be created");
    assert_eq!(registry.len(), 1, "should register one source");
}
