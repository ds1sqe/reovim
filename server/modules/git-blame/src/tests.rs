use {
    super::*,
    reovim_driver_display::{GutterRenderer, GutterRendererKey, GutterRendererRegistry},
    reovim_driver_git::{GitProvider, GitProviderStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ProbeResult},
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
    fn branches(&self, _: &std::path::Path) -> Vec<reovim_driver_git::types::BranchInfo> {
        vec![]
    }
    fn status(&self, _: &std::path::Path) -> Vec<reovim_driver_git::types::StatusEntry> {
        vec![]
    }
    fn log(
        &self,
        _: &std::path::Path,
        _: &str,
        _: usize,
    ) -> Vec<reovim_driver_git::types::LogEntry> {
        vec![]
    }
    fn stash_list(&self, _: &std::path::Path) -> Vec<reovim_driver_git::types::StashEntry> {
        vec![]
    }
    fn diff_hunks(&self, _: &std::path::Path) -> Vec<reovim_driver_git::types::DiffHunk> {
        vec![]
    }
}

/// No default renderer registered → nothing registered, returns Success.
#[test]
fn test_init_no_default_renderer() {
    let mut module = GitBlameModule::new();
    let ctx = ModuleContext::default();
    assert_eq!(module.init(&ctx), ProbeResult::Success);
}

/// Default renderer exists but no git provider → returns Success, no source.
#[test]
fn test_init_renderer_no_git_provider() {
    let mut module = GitBlameModule::new();
    let ctx = ModuleContext::default();

    let registry = ctx.services.get_or_create::<GutterRendererRegistry>();
    registry.register(GutterRendererKey::Default, Arc::new(GutterRenderer::new()));

    assert_eq!(module.init(&ctx), ProbeResult::Success);

    let renderer = registry
        .get(&GutterRendererKey::Default)
        .expect("default renderer");
    assert_eq!(renderer.source_count(), 0, "no source without provider");
    assert_eq!(renderer.presenter_count(), 0, "no presenter without provider");
}

/// Default renderer and git provider both exist → registers source and presenter.
#[test]
fn test_init_with_renderer_and_provider() {
    let mut module = GitBlameModule::new();
    let ctx = ModuleContext::default();

    let registry = ctx.services.get_or_create::<GutterRendererRegistry>();
    registry.register(GutterRendererKey::Default, Arc::new(GutterRenderer::new()));

    let store = ctx.services.get_or_create::<GitProviderStore>();
    store.register(Arc::new(StubGitProvider));

    assert_eq!(module.init(&ctx), ProbeResult::Success);

    let renderer = registry
        .get(&GutterRendererKey::Default)
        .expect("default renderer");
    assert_eq!(renderer.source_count(), 1, "should register one source");
    assert_eq!(renderer.presenter_count(), 1, "should register one presenter");
}
