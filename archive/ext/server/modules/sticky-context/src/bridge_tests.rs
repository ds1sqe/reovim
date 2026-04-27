use {
    reovim_driver_text_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_driver_text_syntax::{ContextHierarchy, ScopeKind, ScopeRange},
};

use {super::*, crate::state::StickyContextState, reovim_module_context::ContextSessionState};

fn sample_hierarchy(cursor_line: u32) -> ContextHierarchy {
    ContextHierarchy::new(
        1,
        cursor_line,
        5,
        vec![
            ScopeRange::new(0, 100, ScopeKind::Module, "mod utils", Some("utils".into())),
            ScopeRange::new(5, 50, ScopeKind::Class, "impl Foo", Some("Foo".into())),
            ScopeRange::new(10, 30, ScopeKind::Function, "fn bar", Some("bar".into())),
        ],
    )
}

#[test]
fn test_bridge_kind() {
    let bridge = StickyContextBridge;
    assert_eq!(bridge.kind(), "sticky-context");
}

#[test]
fn test_bridge_scope() {
    let bridge = StickyContextBridge;
    assert_eq!(bridge.scope(), ExtensionScope::Client);
}

#[test]
fn test_bridge_snapshot_no_state() {
    let bridge = StickyContextBridge;
    let extensions = ExtensionMap::new();
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_bridge_snapshot_no_context() {
    let bridge = StickyContextBridge;
    let mut extensions = ExtensionMap::new();
    extensions.get_or_insert::<StickyContextState>();
    // No ContextSessionState, so no headers
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_bridge_snapshot_with_headers() {
    let bridge = StickyContextBridge;
    let mut extensions = ExtensionMap::new();

    // Set up context with cursor at line 20 (viewport_top = cursor line = 20)
    let ctx = extensions.get_or_insert::<ContextSessionState>();
    ctx.set_hierarchy(sample_hierarchy(20));

    extensions.get_or_insert::<StickyContextState>();

    let snap = bridge.snapshot(&extensions).expect("should have snapshot");
    let headers = snap["headers"].as_array().unwrap();
    assert_eq!(headers.len(), 3);
    assert_eq!(headers[0]["text"], "mod utils");
    assert_eq!(headers[1]["text"], "impl Foo");
    assert_eq!(headers[2]["text"], "fn bar");
    assert_eq!(snap["separator"], true);
}

#[test]
fn test_bridge_snapshot_viewport_at_top() {
    let bridge = StickyContextBridge;
    let mut extensions = ExtensionMap::new();

    // Cursor at line 0 — no scopes above viewport
    let ctx = extensions.get_or_insert::<ContextSessionState>();
    ctx.set_hierarchy(sample_hierarchy(0));

    extensions.get_or_insert::<StickyContextState>();

    // viewport_top = hierarchy.line = 0, nothing above
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_bridge_is_active_no_state() {
    let bridge = StickyContextBridge;
    let extensions = ExtensionMap::new();
    assert!(!bridge.is_active(&extensions));
}

#[test]
fn test_bridge_is_active_enabled() {
    let bridge = StickyContextBridge;
    let mut extensions = ExtensionMap::new();
    extensions.get_or_insert::<StickyContextState>();
    assert!(bridge.is_active(&extensions));
}

#[test]
fn test_bridge_is_active_disabled() {
    let bridge = StickyContextBridge;
    let mut extensions = ExtensionMap::new();
    let state = extensions.get_or_insert::<StickyContextState>();
    state.options.enabled = false;
    assert!(!bridge.is_active(&extensions));
}
