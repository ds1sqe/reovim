use reovim_kernel::api::v1::{CommandId, KeybindingRegistration, ModuleId};

use crate::KeybindingStore;

fn test_binding(keys: &'static str) -> KeybindingRegistration {
    KeybindingRegistration {
        keys,
        command_id: CommandId::new(ModuleId::new("test"), "test-cmd"),
        modes: &["normal"],
        description: "Test binding",
        category: None,
        enabled: true,
        priority: 0,
        depends_on: &[],
        flags: reovim_kernel::api::v1::RegistrationFlags::new(),
    }
}

#[test]
fn test_store_new() {
    let store = KeybindingStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_store_add() {
    let store = KeybindingStore::new();
    store.add(test_binding("j"));
    store.add(test_binding("k"));

    assert_eq!(store.len(), 2);
    assert!(!store.is_empty());
}

#[test]
fn test_store_add_all() {
    let store = KeybindingStore::new();
    store.add_all([test_binding("j"), test_binding("k"), test_binding("l")]);

    assert_eq!(store.len(), 3);
}

#[test]
fn test_store_take_keybindings() {
    let store = KeybindingStore::new();
    store.add(test_binding("j"));
    store.add(test_binding("k"));

    let bindings = store.take_keybindings();
    assert_eq!(bindings.len(), 2);
    assert!(store.is_empty()); // Store should be empty after take
}

#[test]
fn test_store_default() {
    let store = KeybindingStore::default();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_store_debug() {
    let store = KeybindingStore::new();
    store.add(test_binding("j"));
    store.add(test_binding("k"));

    let debug = format!("{store:?}");
    assert!(debug.contains("KeybindingStore"));
    assert!(debug.contains("count"));
    assert!(debug.contains('2'));
}

#[test]
fn test_store_take_keybindings_twice() {
    let store = KeybindingStore::new();
    store.add(test_binding("j"));

    let bindings1 = store.take_keybindings();
    assert_eq!(bindings1.len(), 1);

    // Second take should return empty
    let bindings2 = store.take_keybindings();
    assert!(bindings2.is_empty());
}

#[test]
fn test_store_add_after_take() {
    let store = KeybindingStore::new();
    store.add(test_binding("j"));

    let _ = store.take_keybindings();
    assert!(store.is_empty());

    // Can add after take
    store.add(test_binding("k"));
    assert_eq!(store.len(), 1);
}
