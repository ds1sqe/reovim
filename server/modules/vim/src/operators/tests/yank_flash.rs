use {
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
    reovim_kernel::api::v1::BufferId,
};

use super::super::yank_flash::{KIND, YankFlashBridge, YankFlashState};

// =============================================================================
// YankFlashBridge trait impl tests
// =============================================================================

#[test]
fn test_kind() {
    let bridge = YankFlashBridge;
    assert_eq!(bridge.kind(), "yank-flash");
    assert_eq!(bridge.kind(), KIND);
}

#[test]
fn test_scope_is_client() {
    let bridge = YankFlashBridge;
    assert_eq!(bridge.scope(), ExtensionScope::Client);
}

#[test]
fn test_snapshot_returns_none_when_no_state() {
    let bridge = YankFlashBridge;
    let extensions = ExtensionMap::new();
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_snapshot_returns_none_when_sequence_zero() {
    let bridge = YankFlashBridge;
    let mut extensions = ExtensionMap::new();
    // Insert default state (sequence = 0)
    let _ = extensions.get_or_insert::<YankFlashState>();
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn test_snapshot_returns_json_when_state_present() {
    let bridge = YankFlashBridge;
    let mut extensions = ExtensionMap::new();

    let state = extensions.get_or_insert::<YankFlashState>();
    state.record(BufferId::from_raw(5), 10, 3, 15, 8, true);

    let json = bridge.snapshot(&extensions).expect("should have snapshot");
    assert_eq!(json["bufferId"], 5);
    assert_eq!(json["startLine"], 10);
    assert_eq!(json["endLine"], 15);
    assert_eq!(json["startCol"], 3);
    assert_eq!(json["endCol"], 8);
    assert!(json["isLinewise"].as_bool().unwrap());
    assert_eq!(json["sequence"], 1);
}

#[test]
fn test_snapshot_characterwise() {
    let bridge = YankFlashBridge;
    let mut extensions = ExtensionMap::new();

    let state = extensions.get_or_insert::<YankFlashState>();
    state.record(BufferId::from_raw(2), 0, 5, 0, 10, false);

    let json = bridge.snapshot(&extensions).expect("should have snapshot");
    assert_eq!(json["bufferId"], 2);
    assert_eq!(json["startLine"], 0);
    assert_eq!(json["endLine"], 0);
    assert_eq!(json["startCol"], 5);
    assert_eq!(json["endCol"], 10);
    assert!(!json["isLinewise"].as_bool().unwrap());
    assert_eq!(json["sequence"], 1);
}

#[test]
fn test_is_active_false_when_no_state() {
    let bridge = YankFlashBridge;
    let extensions = ExtensionMap::new();
    assert!(!bridge.is_active(&extensions));
}

#[test]
fn test_is_active_false_when_sequence_zero() {
    let bridge = YankFlashBridge;
    let mut extensions = ExtensionMap::new();
    let _ = extensions.get_or_insert::<YankFlashState>();
    assert!(!bridge.is_active(&extensions));
}

#[test]
fn test_is_active_true_when_state_present() {
    let bridge = YankFlashBridge;
    let mut extensions = ExtensionMap::new();

    let state = extensions.get_or_insert::<YankFlashState>();
    state.record(BufferId::from_raw(1), 0, 0, 5, 0, true);

    assert!(bridge.is_active(&extensions));
}

// =============================================================================
// YankFlashState tests
// =============================================================================

#[test]
fn test_state_create_defaults() {
    use reovim_driver_session::SessionExtension;
    let state = YankFlashState::create();
    assert_eq!(state.buffer_id, BufferId::from_raw(0));
    assert_eq!(state.start_line, 0);
    assert_eq!(state.end_line, 0);
    assert_eq!(state.start_col, 0);
    assert_eq!(state.end_col, 0);
    assert!(!state.is_linewise);
    assert_eq!(state.sequence, 0);
}

#[test]
fn test_state_record_updates_fields() {
    use reovim_driver_session::SessionExtension;
    let mut state = YankFlashState::create();

    state.record(BufferId::from_raw(3), 5, 2, 10, 7, true);

    assert_eq!(state.buffer_id, BufferId::from_raw(3));
    assert_eq!(state.start_line, 5);
    assert_eq!(state.end_line, 10);
    assert_eq!(state.start_col, 2);
    assert_eq!(state.end_col, 7);
    assert!(state.is_linewise);
    assert_eq!(state.sequence, 1);
}

#[test]
fn test_sequence_increments() {
    use reovim_driver_session::SessionExtension;
    let mut state = YankFlashState::create();
    assert_eq!(state.sequence, 0);

    state.record(BufferId::from_raw(1), 0, 0, 0, 0, false);
    assert_eq!(state.sequence, 1);

    state.record(BufferId::from_raw(1), 0, 0, 0, 0, false);
    assert_eq!(state.sequence, 2);

    state.record(BufferId::from_raw(2), 5, 0, 10, 0, true);
    assert_eq!(state.sequence, 3);
}

#[test]
fn test_sequence_increments_on_successive_insertions() {
    let bridge = YankFlashBridge;
    let mut extensions = ExtensionMap::new();

    let state = extensions.get_or_insert::<YankFlashState>();
    state.record(BufferId::from_raw(1), 0, 0, 5, 0, true);
    let json1 = bridge.snapshot(&extensions).unwrap();
    assert_eq!(json1["sequence"], 1);

    let state = extensions.get_or_insert::<YankFlashState>();
    state.record(BufferId::from_raw(1), 0, 0, 5, 0, true);
    let json2 = bridge.snapshot(&extensions).unwrap();
    assert_eq!(json2["sequence"], 2);

    let state = extensions.get_or_insert::<YankFlashState>();
    state.record(BufferId::from_raw(2), 3, 1, 8, 4, false);
    let json3 = bridge.snapshot(&extensions).unwrap();
    assert_eq!(json3["sequence"], 3);
}

#[test]
fn test_state_debug() {
    use reovim_driver_session::SessionExtension;
    let state = YankFlashState::create();
    let debug = format!("{state:?}");
    assert!(debug.contains("YankFlashState"));
    assert!(debug.contains("sequence"));
}
