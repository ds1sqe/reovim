use super::super::insert::*;

#[test]
fn test_bindings_not_empty() {
    let b = bindings();
    assert!(!b.is_empty());
}

#[test]
fn test_all_bindings_in_insert_mode() {
    for binding in bindings() {
        assert!(
            binding.modes.contains(&"vim:insert"),
            "Binding '{}' should be in vim:insert mode",
            binding.keys
        );
    }
}

#[test]
fn test_exit_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"<Esc>"), "Missing '<Esc>' binding");
    assert!(keys.contains(&"<C-c>"), "Missing '<C-c>' binding");
    assert!(keys.contains(&"<C-[>"), "Missing '<C-[>' binding");
}

#[test]
fn test_navigation_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"<Left>"), "Missing '<Left>' binding");
    assert!(keys.contains(&"<Right>"), "Missing '<Right>' binding");
    assert!(keys.contains(&"<Up>"), "Missing '<Up>' binding");
    assert!(keys.contains(&"<Down>"), "Missing '<Down>' binding");
    assert!(keys.contains(&"<Home>"), "Missing '<Home>' binding");
    assert!(keys.contains(&"<End>"), "Missing '<End>' binding");
}

#[test]
fn test_deletion_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"<BS>"), "Missing '<BS>' binding");
    assert!(keys.contains(&"<Del>"), "Missing '<Del>' binding");
    assert!(keys.contains(&"<C-h>"), "Missing '<C-h>' binding");
    assert!(keys.contains(&"<C-w>"), "Missing '<C-w>' binding");
    assert!(keys.contains(&"<C-u>"), "Missing '<C-u>' binding");
}

#[test]
fn test_insert_operation_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"<CR>"), "Missing '<CR>' binding");
    assert!(keys.contains(&"<Tab>"), "Missing '<Tab>' binding");
}

#[test]
fn test_completion_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"<C-n>"), "Missing '<C-n>' binding");
    assert!(keys.contains(&"<C-p>"), "Missing '<C-p>' binding");
    assert!(keys.contains(&"<C-Space>"), "Missing '<C-Space>' binding");
}

#[test]
fn test_all_bindings_have_description() {
    for binding in bindings() {
        assert!(
            !binding.description.is_empty(),
            "Binding '{}' should have a description",
            binding.keys
        );
    }
}

#[test]
fn test_all_bindings_have_category() {
    for binding in bindings() {
        assert!(
            binding.category.is_some(),
            "Binding '{}' should have a category",
            binding.keys
        );
    }
}
