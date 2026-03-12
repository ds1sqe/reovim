use {
    super::*,
    reovim_driver_git::{
        GitProvider,
        types::{BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry},
    },
    std::path::{Path, PathBuf},
};

struct MockGit;
impl GitProvider for MockGit {
    fn current_branch(&self, _cwd: &Path) -> Option<String> {
        Some("main".to_owned())
    }
    fn branches(&self, _cwd: &Path) -> Vec<BranchInfo> {
        vec![
            BranchInfo {
                name: "main".to_owned(),
                is_current: true,
                upstream: Some("origin/main".to_owned()),
            },
            BranchInfo {
                name: "feature/foo".to_owned(),
                is_current: false,
                upstream: None,
            },
        ]
    }
    fn status(&self, _cwd: &Path) -> Vec<StatusEntry> {
        vec![]
    }
    fn log(&self, _cwd: &Path, _q: &str, _l: usize) -> Vec<LogEntry> {
        vec![]
    }
    fn stash_list(&self, _cwd: &Path) -> Vec<StashEntry> {
        vec![]
    }
    fn diff_hunks(&self, _path: &Path) -> Vec<DiffHunk> {
        vec![]
    }
}

fn services_with_git() -> ServiceRegistry {
    let services = ServiceRegistry::new();
    let store = services.get_or_create::<GitProviderStore>();
    store.register(Arc::new(MockGit));
    services
}

fn empty_ctx() -> PickerContext {
    PickerContext {
        cwd: PathBuf::from("."),
        query: String::new(),
        buffers: vec![],
        commands: vec![],
        options: vec![],
    }
}

#[test]
fn name_and_title() {
    let picker = GitBranchesPicker::new();
    assert_eq!(picker.name(), "git-branches");
    assert_eq!(picker.title(), "Git Branches");
}

#[test]
fn is_static() {
    assert!(GitBranchesPicker::new().is_static());
}

#[test]
fn items_no_provider() {
    let picker = GitBranchesPicker::new();
    let services = ServiceRegistry::new();
    assert!(picker.items(&empty_ctx(), &services).is_empty());
}

#[test]
fn items_with_provider() {
    let picker = GitBranchesPicker::new();
    let services = services_with_git();
    let items = picker.items(&empty_ctx(), &services);

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].display, "main");
    assert_eq!(items[0].icon, Some('*'));
    assert_eq!(items[0].detail.as_deref(), Some("origin/main"));

    assert_eq!(items[1].display, "feature/foo");
    assert!(items[1].icon.is_none());
    assert!(items[1].detail.is_none());
}

#[test]
fn on_select_closes() {
    let picker = GitBranchesPicker::new();
    let item = PickerItem {
        display: "main".to_owned(),
        detail: None,
        data: PickerData::Text("main".to_owned()),
        icon: None,
    };
    assert!(matches!(picker.on_select(&item), PickerAction::Close));
}

#[test]
fn on_select_wrong_data_closes() {
    let picker = GitBranchesPicker::new();
    let item = PickerItem {
        display: "x".to_owned(),
        detail: None,
        data: PickerData::BufferId(1),
        icon: None,
    };
    assert!(matches!(picker.on_select(&item), PickerAction::Close));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn default_impl() {
    let picker = GitBranchesPicker::default();
    assert_eq!(picker.name(), "git-branches");
}

#[test]
fn no_preview() {
    let picker = GitBranchesPicker::new();
    let item = PickerItem {
        display: "x".to_owned(),
        detail: None,
        data: PickerData::Text("x".to_owned()),
        icon: None,
    };
    assert!(picker.preview(&item, &ServiceRegistry::new()).is_none());
}

// -- Module tests --

#[test]
fn module_id() {
    assert_eq!(PickerGitBranchesModule::new().id().as_str(), "picker-git-branches");
}

#[test]
fn module_name() {
    assert_eq!(PickerGitBranchesModule::new().name(), "Git Branches Picker");
}

#[test]
fn module_version() {
    let v = PickerGitBranchesModule::new().version();
    assert_eq!((v.major, v.minor), (0, 1));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    assert_eq!(PickerGitBranchesModule::default().id().as_str(), "picker-git-branches");
}

#[test]
fn module_exit() {
    assert!(PickerGitBranchesModule::new().exit().is_ok());
}

#[test]
fn module_init_registers_picker() {
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        reovim_kernel::api::v1::KernelContext::default(),
        services.clone(),
        PathBuf::from("/tmp"),
        PathBuf::from("/tmp"),
    );
    let mut module = PickerGitBranchesModule::new();
    assert!(matches!(module.init(&ctx), ProbeResult::Success));
    assert!(
        services
            .get::<PickerRegistry>()
            .unwrap()
            .get("git-branches")
            .is_some()
    );
}

#[test]
fn items_empty_store() {
    let services = ServiceRegistry::new();
    let _ = services.get_or_create::<GitProviderStore>();
    let picker = GitBranchesPicker::new();
    assert!(picker.items(&empty_ctx(), &services).is_empty());
}
