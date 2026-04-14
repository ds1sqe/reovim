use super::*;

use reovim_driver_text_session::ExtensionMap;

use crate::state::{IndentGuide, IndentGuideState};

#[test]
fn test_bridge_kind() {
    let bridge = IndentGuideBridge;
    assert_eq!(bridge.kind(), "indent-guide");
}

#[test]
fn test_bridge_scope() {
    let bridge = IndentGuideBridge;
    assert_eq!(bridge.scope(), ExtensionScope::Client);
}

#[test]
fn test_bridge_is_active_no_state() {
    let bridge = IndentGuideBridge;
    let extensions = ExtensionMap::new();
    assert!(!bridge.is_active(&extensions));
}

#[test]
fn test_bridge_is_active_disabled() {
    let bridge = IndentGuideBridge;
    let mut extensions = ExtensionMap::new();
    let _ = extensions.get_or_insert::<IndentGuideState>();
    assert!(!bridge.is_active(&extensions));
}

#[test]
fn test_bridge_is_active_enabled() {
    let bridge = IndentGuideBridge;
    let mut extensions = ExtensionMap::new();
    let state = extensions.get_or_insert::<IndentGuideState>();
    state.options.enabled = true;
    assert!(bridge.is_active(&extensions));
}

#[test]
fn test_bridge_snapshot_no_state() {
    let bridge = IndentGuideBridge;
    let extensions = ExtensionMap::new();
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_bridge_snapshot_disabled() {
    let bridge = IndentGuideBridge;
    let mut extensions = ExtensionMap::new();
    let _ = extensions.get_or_insert::<IndentGuideState>();
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_bridge_snapshot_enabled_no_guides() {
    let bridge = IndentGuideBridge;
    let mut extensions = ExtensionMap::new();
    let state = extensions.get_or_insert::<IndentGuideState>();
    state.options.enabled = true;

    let snap = bridge
        .snapshot(&extensions)
        .expect("should produce snapshot");
    let guides = snap["guides"].as_array().expect("guides array");
    assert!(guides.is_empty());
    assert_eq!(snap["active"], true);
}

#[test]
fn test_bridge_snapshot_with_guides() {
    let bridge = IndentGuideBridge;
    let mut extensions = ExtensionMap::new();
    let state = extensions.get_or_insert::<IndentGuideState>();
    state.options.enabled = true;
    state.guides.push(IndentGuide::new(0, 0, true));
    state.guides.push(IndentGuide::new(1, 0, false));

    let snap = bridge
        .snapshot(&extensions)
        .expect("should produce snapshot");
    let guides = snap["guides"].as_array().expect("guides array");
    assert_eq!(guides.len(), 2);
    assert_eq!(guides[0]["line"], 0);
    assert_eq!(guides[0]["level"], 0);
    assert_eq!(guides[0]["active"], true);
    assert_eq!(guides[1]["active"], false);
    assert_eq!(snap["guide_char"], "\u{2502}");
    assert_eq!(snap["tab_size"], 4);
}
