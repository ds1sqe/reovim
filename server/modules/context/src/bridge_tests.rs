use {
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_driver_syntax::{ContextHierarchy, ScopeKind, ScopeRange},
};

use {super::*, crate::state::ContextSessionState};

fn sample_hierarchy() -> ContextHierarchy {
    ContextHierarchy::new(
        1,
        10,
        5,
        vec![
            ScopeRange::new(0, 100, ScopeKind::Module, "mod utils", Some("utils".into())),
            ScopeRange::new(5, 20, ScopeKind::Function, "fn bar", Some("bar".into())),
        ],
    )
}

#[test]
fn test_bridge_kind() {
    let bridge = ContextBridge;
    assert_eq!(bridge.kind(), "context");
}

#[test]
fn test_bridge_scope() {
    let bridge = ContextBridge;
    assert_eq!(bridge.scope(), ExtensionScope::Client);
}

#[test]
fn test_bridge_snapshot_no_state() {
    let bridge = ContextBridge;
    let extensions = ExtensionMap::new();
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_bridge_snapshot_empty_hierarchy() {
    let bridge = ContextBridge;
    let mut extensions = ExtensionMap::new();

    let state = extensions.get_or_insert::<ContextSessionState>();
    state.set_hierarchy(ContextHierarchy::empty());

    let snap = bridge.snapshot(&extensions).expect("should have snapshot");
    assert_eq!(snap["active"], false);
    assert!(snap["items"].as_array().unwrap().is_empty());
}

#[test]
fn test_bridge_snapshot_with_items() {
    let bridge = ContextBridge;
    let mut extensions = ExtensionMap::new();

    let state = extensions.get_or_insert::<ContextSessionState>();
    state.set_hierarchy(sample_hierarchy());

    let snap = bridge.snapshot(&extensions).expect("should have snapshot");
    assert_eq!(snap["active"], true);
    assert_eq!(snap["breadcrumb"], "mod utils > fn bar");

    let items = snap["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["kind"], "mod");
    assert_eq!(items[0]["display_text"], "mod utils");
    assert_eq!(items[1]["kind"], "fn");
    assert_eq!(items[1]["display_text"], "fn bar");
}

#[test]
fn test_bridge_is_active_no_state() {
    let bridge = ContextBridge;
    let extensions = ExtensionMap::new();
    assert!(!bridge.is_active(&extensions));
}

#[test]
fn test_bridge_is_active_empty_hierarchy() {
    let bridge = ContextBridge;
    let mut extensions = ExtensionMap::new();

    let state = extensions.get_or_insert::<ContextSessionState>();
    state.set_hierarchy(ContextHierarchy::empty());

    assert!(!bridge.is_active(&extensions));
}

#[test]
fn test_bridge_is_active_with_items() {
    let bridge = ContextBridge;
    let mut extensions = ExtensionMap::new();

    let state = extensions.get_or_insert::<ContextSessionState>();
    state.set_hierarchy(sample_hierarchy());

    assert!(bridge.is_active(&extensions));
}
