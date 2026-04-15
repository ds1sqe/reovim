use {
    super::*,
    reovim_driver_git::{
        GitProvider,
        types::{BranchInfo, DiffHunk, FileStatus, LogEntry, StashEntry, StatusEntry},
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
        vec![
            StatusEntry {
                path: PathBuf::from("src/main.rs"),
                index_status: FileStatus::Modified,
                worktree_status: FileStatus::Unmodified,
            },
            StatusEntry {
                path: PathBuf::from("new_file.rs"),
                index_status: FileStatus::Untracked,
                worktree_status: FileStatus::Untracked,
            },
        ]
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

fn ctx_with_cwd() -> PickerContext {
    PickerContext {
        cwd: PathBuf::from("/proj"),
        query: String::new(),
        buffers: vec![],
        commands: vec![],
        options: vec![],
    }
}

#[test]
fn name_and_title() {
    let picker = GitStatusPicker::new();
    assert_eq!(picker.name(), "git-status");
    assert_eq!(picker.title(), "Git Status");
}

#[test]
fn is_static() {
    assert!(GitStatusPicker::new().is_static());
}

#[test]
fn items_no_provider() {
    let picker = GitStatusPicker::new();
    let services = ServiceRegistry::new();
    assert!(picker.items(&ctx_with_cwd(), &services).is_empty());
}

#[test]
fn items_with_provider() {
    let picker = GitStatusPicker::new();
    let services = services_with_git();
    let items = picker.items(&ctx_with_cwd(), &services);

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].display, "src/main.rs");
    assert_eq!(items[0].detail.as_deref(), Some("M "));
    assert_eq!(items[0].icon, Some('~'));

    assert_eq!(items[1].display, "new_file.rs");
    assert_eq!(items[1].detail.as_deref(), Some("??"));
    assert_eq!(items[1].icon, Some('?'));
}

#[test]
fn on_select_opens_file() {
    let picker = GitStatusPicker::new();
    let item = PickerItem {
        display: "main.rs".to_owned(),
        detail: None,
        data: PickerData::FilePath(PathBuf::from("/proj/main.rs")),
        icon: None,
    };
    assert!(matches!(picker.on_select(&item), PickerAction::OpenFile(_)));
}

#[test]
fn on_select_wrong_data_closes() {
    let picker = GitStatusPicker::new();
    let item = PickerItem {
        display: "x".to_owned(),
        detail: None,
        data: PickerData::Text("x".to_owned()),
        icon: None,
    };
    assert!(matches!(picker.on_select(&item), PickerAction::Close));
}

#[test]
fn status_char_all_variants() {
    assert_eq!(status_char(FileStatus::Modified), 'M');
    assert_eq!(status_char(FileStatus::Added), 'A');
    assert_eq!(status_char(FileStatus::Deleted), 'D');
    assert_eq!(status_char(FileStatus::Renamed), 'R');
    assert_eq!(status_char(FileStatus::Copied), 'C');
    assert_eq!(status_char(FileStatus::Untracked), '?');
    assert_eq!(status_char(FileStatus::Ignored), '!');
    assert_eq!(status_char(FileStatus::Unmodified), ' ');
}

#[test]
fn status_icon_variants() {
    assert_eq!(status_icon(FileStatus::Added, FileStatus::Unmodified), Some('+'));
    assert_eq!(status_icon(FileStatus::Unmodified, FileStatus::Deleted), Some('-'));
    assert_eq!(status_icon(FileStatus::Modified, FileStatus::Unmodified), Some('~'));
    assert_eq!(status_icon(FileStatus::Untracked, FileStatus::Untracked), Some('?'));
    assert_eq!(status_icon(FileStatus::Unmodified, FileStatus::Unmodified), None);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn default_impls() {
    assert_eq!(GitStatusPicker::default().name(), "git-status");
    assert_eq!(PickerGitStatusModule::default().id().as_str(), "picker-git-status");
}

#[test]
fn no_preview() {
    let picker = GitStatusPicker::new();
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
    let m = PickerGitStatusModule::new();
    assert_eq!(m.id().as_str(), "picker-git-status");
    assert_eq!(m.name(), "Git Status Picker");
    assert_eq!((m.version().major, m.version().minor), (0, 1));
}

#[test]
fn module_exit() {
    assert!(PickerGitStatusModule::new().exit().is_ok());
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
    let mut module = PickerGitStatusModule::new();
    assert!(matches!(module.init(&ctx), ProbeResult::Success));
    assert!(
        services
            .get::<PickerRegistry>()
            .unwrap()
            .get("git-status")
            .is_some()
    );
}

#[test]
fn items_empty_store() {
    let services = ServiceRegistry::new();
    let _ = services.get_or_create::<GitProviderStore>();
    assert!(
        GitStatusPicker::new()
            .items(&ctx_with_cwd(), &services)
            .is_empty()
    );
}
