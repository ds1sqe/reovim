use reovim_driver_text_session::{ExtensionMap, bridges::ExtensionStateBridge};

use {
    super::*,
    crate::state::{ModuleEntry, ModuleFilter, ModuleManagerState, ModuleStatus},
};

#[test]
fn test_bridge_kind() {
    assert_eq!(ModuleManagerBridge.kind(), "module-manager");
}

#[test]
fn test_bridge_scope_is_client() {
    assert_eq!(ModuleManagerBridge.scope(), ExtensionScope::Client);
}

#[test]
fn test_snapshot_inactive() {
    let mut ext = ExtensionMap::new();
    ext.get_or_insert::<ModuleManagerState>();
    let json = ModuleManagerBridge.snapshot(&ext).unwrap();
    assert_eq!(json["active"], false);
}

#[test]
fn test_snapshot_active_with_modules() {
    let mut ext = ExtensionMap::new();
    let state = ext.get_or_insert::<ModuleManagerState>();
    state.active = true;
    state.modules = vec![
        ModuleEntry {
            id: "vim".into(),
            version: "0.10.0".into(),
            status: ModuleStatus::Loaded,
            reason: None,
        },
        ModuleEntry {
            id: "broken".into(),
            version: "0.1.0".into(),
            status: ModuleStatus::Failed,
            reason: Some("init error".into()),
        },
    ];

    let json = ModuleManagerBridge.snapshot(&ext).unwrap();
    assert_eq!(json["active"], true);
    assert_eq!(json["total"], 2);
    assert_eq!(json["filtered"], 2);
    assert_eq!(json["filter"], "All");
    assert_eq!(json["selected"], 0);

    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], "vim");
    assert_eq!(items[0]["indicator"], "[*]");
    assert_eq!(items[1]["id"], "broken");
    assert_eq!(items[1]["reason"], "init error");
}

#[test]
fn test_snapshot_with_filter() {
    let mut ext = ExtensionMap::new();
    let state = ext.get_or_insert::<ModuleManagerState>();
    state.active = true;
    state.filter = ModuleFilter::Failed;
    state.modules = vec![
        ModuleEntry {
            id: "vim".into(),
            version: "0.10.0".into(),
            status: ModuleStatus::Loaded,
            reason: None,
        },
        ModuleEntry {
            id: "broken".into(),
            version: "0.1.0".into(),
            status: ModuleStatus::Failed,
            reason: Some("crash".into()),
        },
    ];

    let json = ModuleManagerBridge.snapshot(&ext).unwrap();
    assert_eq!(json["total"], 2);
    assert_eq!(json["filtered"], 1);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "broken");
}

#[test]
fn test_snapshot_no_state() {
    let ext = ExtensionMap::new();
    assert!(ModuleManagerBridge.snapshot(&ext).is_none());
}

#[test]
fn test_is_active_false_when_no_state() {
    let ext = ExtensionMap::new();
    assert!(!ModuleManagerBridge.is_active(&ext));
}

#[test]
fn test_is_active_true_when_active() {
    let mut ext = ExtensionMap::new();
    let state = ext.get_or_insert::<ModuleManagerState>();
    state.active = true;
    assert!(ModuleManagerBridge.is_active(&ext));
}
