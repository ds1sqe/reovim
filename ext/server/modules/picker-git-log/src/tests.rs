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
        vec![
            LogEntry {
                hash: "abc123def456".to_owned(),
                short_hash: "abc123d".to_owned(),
                author: "Alice".to_owned(),
                date: "2025-01-01".to_owned(),
                message: "feat: add feature".to_owned(),
            },
            LogEntry {
                hash: "789012fed321".to_owned(),
                short_hash: "789012f".to_owned(),
                author: "Bob".to_owned(),
                date: "2025-01-02".to_owned(),
                message: "fix: bug fix".to_owned(),
            },
        ]
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
    let picker = GitLogPicker::new();
    assert_eq!(picker.name(), "git-log");
    assert_eq!(picker.title(), "Git Log");
}

#[test]
fn is_dynamic() {
    assert!(!GitLogPicker::new().is_static());
}

#[test]
fn items_no_provider() {
    let picker = GitLogPicker::new();
    let services = ServiceRegistry::new();
    assert!(picker.items(&empty_ctx(), &services).is_empty());
}

#[test]
fn items_with_provider() {
    let picker = GitLogPicker::new();
    let services = services_with_git();
    let items = picker.items(&empty_ctx(), &services);

    assert_eq!(items.len(), 2);
    assert!(items[0].display.contains("abc123d"));
    assert!(items[0].display.contains("add feature"));
    assert!(items[0].detail.as_ref().unwrap().contains("Alice"));

    assert!(items[1].display.contains("789012f"));
    assert!(items[1].detail.as_ref().unwrap().contains("Bob"));
}

#[test]
fn on_select_closes() {
    let picker = GitLogPicker::new();
    let item = PickerItem {
        display: "abc".to_owned(),
        detail: None,
        data: PickerData::Text("abc123".to_owned()),
        icon: None,
    };
    assert!(matches!(picker.on_select(&item), PickerAction::Close));
}

#[test]
fn on_select_wrong_data_closes() {
    let picker = GitLogPicker::new();
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
    assert_eq!(GitLogPicker::default().name(), "git-log");
    assert_eq!(PickerGitLogModule::default().id().as_str(), "picker-git-log");
}

#[test]
fn no_preview() {
    let picker = GitLogPicker::new();
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
    let m = PickerGitLogModule::new();
    assert_eq!(m.id().as_str(), "picker-git-log");
    assert_eq!(m.name(), "Git Log Picker");
    assert_eq!((m.version().major, m.version().minor), (0, 1));
}

#[test]
fn module_exit() {
    assert!(PickerGitLogModule::new().exit().is_ok());
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
    let mut module = PickerGitLogModule::new();
    assert!(matches!(module.init(&ctx), ProbeResult::Success));
    assert!(
        services
            .get::<PickerRegistry>()
            .unwrap()
            .get("git-log")
            .is_some()
    );
}

#[test]
fn items_empty_store() {
    let services = ServiceRegistry::new();
    let _ = services.get_or_create::<GitProviderStore>();
    assert!(
        GitLogPicker::new()
            .items(&empty_ctx(), &services)
            .is_empty()
    );
}
