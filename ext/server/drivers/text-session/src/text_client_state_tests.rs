use {
    reovim_domain_text::Position,
    reovim_kernel::api::v1::BufferId,
    reovim_subsys_session::{ExtensionMap, SessionExtension},
};

use super::*;

#[test]
fn test_create_default_state() {
    let state = TextClientState::new();
    assert!(state.jumplist().is_empty());
    assert!(state.local_marks().list_local().is_empty());
}

#[test]
fn test_default_trait() {
    let state = TextClientState::default();
    assert!(state.jumplist().is_empty());
}

#[test]
fn test_session_extension_create() {
    let state = TextClientState::create();
    assert!(state.jumplist().is_empty());
    assert!(state.local_marks().list_local().is_empty());
}

#[test]
fn test_register_access() {
    let mut state = TextClientState::new();
    let _registers = state.registers();
    let _registers_mut = state.registers_mut();
}

#[test]
fn test_clipboard_history_access() {
    let mut state = TextClientState::new();
    let _history = state.clipboard_history();
    let _history_mut = state.clipboard_history_mut();
}

#[test]
fn test_local_marks_access() {
    let mut state = TextClientState::new();
    state.local_marks_mut().set_local('a', Position::new(10, 5));
    let pos = state.local_marks().get_local('a');
    assert_eq!(pos, Some(Position::new(10, 5)));
}

#[test]
fn test_jumplist_access() {
    let mut state = TextClientState::new();
    let buf_id = BufferId::new();
    state
        .jumplist_mut()
        .push(crate::JumpEntry::new(buf_id, Position::new(0, 0)));
    assert_eq!(state.jumplist().len(), 1);
}

#[test]
fn test_extension_map_roundtrip() {
    let mut map = ExtensionMap::new();

    // First access creates the extension
    let state = map.get_or_insert::<TextClientState>();
    state.local_marks_mut().set_local('b', Position::new(42, 7));

    // Retrieve and verify
    let state = map.get::<TextClientState>().unwrap();
    assert_eq!(state.local_marks().get_local('b'), Some(Position::new(42, 7)));
}

#[test]
fn test_extension_map_contains() {
    let mut map = ExtensionMap::new();
    assert!(!map.contains::<TextClientState>());
    map.get_or_insert::<TextClientState>();
    assert!(map.contains::<TextClientState>());
}

#[test]
fn test_extension_map_mutable_access() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<TextClientState>();

    let state = map.get_mut::<TextClientState>().unwrap();
    let buf_id = BufferId::new();
    state
        .jumplist_mut()
        .push(crate::JumpEntry::new(buf_id, Position::new(5, 0)));

    let state = map.get::<TextClientState>().unwrap();
    assert_eq!(state.jumplist().len(), 1);
}
