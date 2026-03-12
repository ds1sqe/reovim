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
        None
    }
    fn branches(&self, _cwd: &Path) -> Vec<BranchInfo> {
        vec![]
    }
    fn status(&self, _cwd: &Path) -> Vec<StatusEntry> {
        vec![]
    }
    fn log(&self, _cwd: &Path, _q: &str, _l: usize) -> Vec<LogEntry> {
        vec![]
    }
    fn stash_list(&self, _cwd: &Path) -> Vec<StashEntry> {
        vec![
            StashEntry {
                index: 0,
                message: "WIP on main: abc123 feat: add feature".to_owned(),
            },
            StashEntry {
                index: 1,
                message: "WIP on develop: def456 fix: bug".to_owned(),
            },
        ]
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
    let picker = GitStashPicker::new();
    assert_eq!(picker.name(), "git-stash");
    assert_eq!(picker.title(), "Git Stash");
}

#[test]
fn is_static() {
    assert!(GitStashPicker::new().is_static());
}

#[test]
fn items_no_provider() {
    let picker = GitStashPicker::new();
    let services = ServiceRegistry::new();
    assert!(picker.items(&empty_ctx(), &services).is_empty());
}

#[test]
fn items_with_provider() {
    let picker = GitStashPicker::new();
    let services = services_with_git();
    let items = picker.items(&empty_ctx(), &services);

    assert_eq!(items.len(), 2);
    assert!(items[0].display.contains("WIP on main"));
    assert_eq!(items[0].detail.as_deref(), Some("stash@{0}"));

    assert!(items[1].display.contains("WIP on develop"));
    assert_eq!(items[1].detail.as_deref(), Some("stash@{1}"));
}

#[test]
fn on_select_closes() {
    let picker = GitStashPicker::new();
    let item = PickerItem {
        display: "stash".to_owned(),
        detail: None,
        data: PickerData::Text("stash@{0}".to_owned()),
        icon: None,
    };
    assert!(matches!(picker.on_select(&item), PickerAction::Close));
}

#[test]
fn on_select_wrong_data_closes() {
    let picker = GitStashPicker::new();
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
fn default_impls() {
    assert_eq!(GitStashPicker::default().name(), "git-stash");
    assert_eq!(PickerGitStashModule::default().id().as_str(), "picker-git-stash");
}

#[test]
fn no_preview() {
    let picker = GitStashPicker::new();
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
fn module_metadata() {
    let m = PickerGitStashModule::new();
    assert_eq!(m.id().as_str(), "picker-git-stash");
    assert_eq!(m.name(), "Git Stash Picker");
    assert_eq!((m.version().major, m.version().minor), (0, 1));
}

#[test]
fn module_exit() {
    assert!(PickerGitStashModule::new().exit().is_ok());
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
    let mut module = PickerGitStashModule::new();
    assert!(matches!(module.init(&ctx), ProbeResult::Success));
    assert!(
        services
            .get::<PickerRegistry>()
            .unwrap()
            .get("git-stash")
            .is_some()
    );
}

#[test]
fn items_empty_store() {
    let services = ServiceRegistry::new();
    let _ = services.get_or_create::<GitProviderStore>();
    assert!(
        GitStashPicker::new()
            .items(&empty_ctx(), &services)
            .is_empty()
    );
}
