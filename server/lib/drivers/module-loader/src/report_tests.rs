use {super::*, reovim_kernel::api::v1::ModuleId, std::path::PathBuf};

#[test]
fn new_report_is_empty() {
    let report = ModuleLoadReport::new();
    assert!(report.loaded.is_empty());
    assert!(report.disabled.is_empty());
    assert!(report.failed.is_empty());
    assert!(report.missing_deps.is_empty());
    assert!(report.config_path.is_none());
    assert!(report.search_paths.is_empty());
    assert!(!report.isolation_active);
}

#[test]
fn default_matches_new() {
    let a = ModuleLoadReport::new();
    let b = ModuleLoadReport::default();
    assert_eq!(a.loaded.len(), b.loaded.len());
    assert_eq!(a.isolation_active, b.isolation_active);
}

#[test]
fn total_count_sums_all_categories() {
    let mut report = ModuleLoadReport::new();
    report.loaded.push(ModuleId::new("a"));
    report.loaded.push(ModuleId::new("b"));
    report.disabled.push(ModuleId::new("c"));
    report.failed.push((ModuleId::new("d"), "boom".to_string()));
    assert_eq!(report.total_count(), 4);
}

#[test]
fn total_count_zero_when_empty() {
    let report = ModuleLoadReport::new();
    assert_eq!(report.total_count(), 0);
}

#[test]
fn debug_format() {
    let mut report = ModuleLoadReport::new();
    report.loaded.push(ModuleId::new("vim"));
    let debug = format!("{report:?}");
    assert!(debug.contains("ModuleLoadReport"));
    assert!(debug.contains("loaded: 1"));
}

#[test]
fn report_with_config_path() {
    let mut report = ModuleLoadReport::new();
    report.config_path = Some(PathBuf::from("/home/user/.config/reovim/modules.toml"));
    assert!(report.config_path.is_some());
}

#[test]
fn report_with_search_paths() {
    let mut report = ModuleLoadReport::new();
    report
        .search_paths
        .push(PathBuf::from("/usr/lib/reovim/modules"));
    report
        .search_paths
        .push(PathBuf::from("/home/user/.local/share/reovim/modules"));
    assert_eq!(report.search_paths.len(), 2);
}

#[test]
fn report_isolation_active() {
    let mut report = ModuleLoadReport::new();
    report.isolation_active = true;
    assert!(report.isolation_active);
}

#[test]
fn report_missing_deps() {
    let mut report = ModuleLoadReport::new();
    report
        .missing_deps
        .push((ModuleId::new("snippet"), ModuleId::new("vim")));
    assert_eq!(report.missing_deps.len(), 1);
    assert_eq!(report.missing_deps[0].0.as_str(), "snippet");
    assert_eq!(report.missing_deps[0].1.as_str(), "vim");
}
