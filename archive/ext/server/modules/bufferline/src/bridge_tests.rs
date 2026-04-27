use reovim_driver_text_session::{ExtensionMap, bridges::ExtensionScope};

use super::*;

// ============================================================================
// Trait method tests
// ============================================================================

#[test]
fn bridge_kind() {
    assert_eq!(PinBridge.kind(), "bufferline");
}

#[test]
fn bridge_scope_shared() {
    assert_eq!(PinBridge.scope(), ExtensionScope::Shared);
}

#[test]
fn bridge_is_active_false_when_no_state() {
    let extensions = ExtensionMap::new();
    assert!(!PinBridge.is_active(&extensions));
}

#[test]
fn bridge_is_active_true_when_state_exists() {
    let mut extensions = ExtensionMap::new();
    let _ = extensions.get_or_insert::<BufferlineState>();
    assert!(PinBridge.is_active(&extensions));
}

// ============================================================================
// snapshot()
// ============================================================================

#[test]
fn snapshot_no_state_returns_none() {
    let extensions = ExtensionMap::new();
    assert!(PinBridge.snapshot(&extensions).is_none());
}

#[test]
fn snapshot_empty_pins() {
    let mut extensions = ExtensionMap::new();
    let _ = extensions.get_or_insert::<BufferlineState>();
    let json = PinBridge.snapshot(&extensions).unwrap();
    assert_eq!(json["type"], "pin_state");
    assert!(json["pins"].as_array().unwrap().is_empty());
}

#[test]
fn snapshot_with_pins() {
    let mut extensions = ExtensionMap::new();
    let state = extensions.get_or_insert::<BufferlineState>();
    state.pin(1);
    state.pin(3);

    let json = PinBridge.snapshot(&extensions).unwrap();
    assert_eq!(json["type"], "pin_state");
    let pins = json["pins"].as_array().unwrap();
    assert_eq!(pins.len(), 2);
    assert_eq!(pins[0], 1);
    assert_eq!(pins[1], 3);
}

// ============================================================================
// tick()
// ============================================================================

#[test]
fn tick_always_returns_false() {
    let mut client = ExtensionMap::new();
    let mut shared = ExtensionMap::new();
    let services = ServiceRegistry::new();
    assert!(!PinBridge.tick(&mut client, &mut shared, &services));
}
