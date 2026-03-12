use {
    super::*,
    reovim_driver_display::statusline::ComponentProviderKey,
    reovim_kernel::api::v1::{Module, ModuleContext, ProbeResult},
    std::sync::Arc,
};

#[test]
fn test_module_id() {
    let module = GitStatuslineModule::new();
    assert_eq!(module.id().as_str(), "git-statusline");
}

#[test]
fn test_module_name() {
    let module = GitStatuslineModule::new();
    assert_eq!(module.name(), "Git Statusline");
}

#[test]
fn test_module_version() {
    let module = GitStatuslineModule::new();
    let v = module.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn test_module_default() {
    let m1 = GitStatuslineModule::new();
    let m2 = GitStatuslineModule;
    assert_eq!(m1.id(), m2.id());
}

#[test]
fn test_exit_succeeds() {
    let mut module = GitStatuslineModule::new();
    assert!(module.exit().is_ok());
}

// ========================================================================
// init() coverage
// ========================================================================

/// No `GitProviderStore` in services → early return, nothing registered.
#[test]
fn test_init_no_git_provider_store() {
    let mut module = GitStatuslineModule::new();
    let ctx = ModuleContext::default();
    assert_eq!(module.init(&ctx), ProbeResult::Success);
}

/// `GitProviderStore` exists but no provider set → early return.
#[test]
fn test_init_git_provider_store_empty() {
    let mut module = GitStatuslineModule::new();
    let ctx = ModuleContext::default();
    ctx.services.register(Arc::new(GitProviderStore::new()));
    assert_eq!(module.init(&ctx), ProbeResult::Success);
}

/// `GitProviderStore` with a provider → registers branch component.
#[test]
fn test_init_registers_branch_component() {
    use {
        reovim_driver_display::statusline::ComponentProviderRegistry,
        reovim_driver_git::GitProvider,
    };

    struct StubGitProvider;
    impl GitProvider for StubGitProvider {
        fn current_branch(&self, _: &std::path::Path) -> Option<String> {
            Some("main".to_string())
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

    let mut module = GitStatuslineModule::new();
    let ctx = ModuleContext::default();

    let store = ctx.services.get_or_create::<GitProviderStore>();
    store.register(Arc::new(StubGitProvider));

    assert_eq!(module.init(&ctx), ProbeResult::Success);

    let registry = ctx.services.get::<ComponentProviderRegistry>();
    assert!(registry.is_some(), "ComponentProviderRegistry should exist");
    let registry = registry.unwrap();
    assert!(
        registry.get(&ComponentProviderKey::new("branch")).is_some(),
        "branch component should be registered"
    );
}
