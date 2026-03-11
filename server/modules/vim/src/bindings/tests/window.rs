use super::super::window::*;

#[test]
fn test_bindings_not_empty() {
    let bindings = bindings();
    assert!(!bindings.is_empty());
}

#[test]
fn test_bindings_all_in_window_mode() {
    for binding in bindings() {
        assert!(
            binding.modes.contains(&"vim:window"),
            "Binding '{}' should be in vim:window mode",
            binding.keys
        );
    }
}

#[test]
fn test_navigation_bindings_exist() {
    let bindings = bindings();
    let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
    assert!(keys.contains(&"h"), "Should have 'h' for focus left");
    assert!(keys.contains(&"j"), "Should have 'j' for focus down");
    assert!(keys.contains(&"k"), "Should have 'k' for focus up");
    assert!(keys.contains(&"l"), "Should have 'l' for focus right");
}

#[test]
fn test_split_bindings_exist() {
    let bindings = bindings();
    let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
    assert!(keys.contains(&"s"), "Should have 's' for horizontal split");
    assert!(keys.contains(&"v"), "Should have 'v' for vertical split");
}

#[test]
fn test_close_bindings_exist() {
    let bindings = bindings();
    let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
    assert!(keys.contains(&"c"), "Should have 'c' for close window");
    assert!(keys.contains(&"o"), "Should have 'o' for close others");
}

#[test]
fn test_resize_bindings_exist() {
    let bindings = bindings();
    let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
    assert!(keys.contains(&"+"), "Should have '+' for increase height");
    assert!(keys.contains(&"-"), "Should have '-' for decrease height");
    assert!(keys.contains(&">"), "Should have '>' for increase width");
    assert!(keys.contains(&"<lt>"), "Should have '<lt>' for decrease width");
    assert!(keys.contains(&"="), "Should have '=' for equalize");
}

#[test]
fn test_arrow_key_alternatives_exist() {
    let bindings = bindings();
    let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
    assert!(keys.contains(&"<Left>"), "Should have '<Left>' for focus left");
    assert!(keys.contains(&"<Down>"), "Should have '<Down>' for focus down");
    assert!(keys.contains(&"<Up>"), "Should have '<Up>' for focus up");
    assert!(keys.contains(&"<Right>"), "Should have '<Right>' for focus right");
}

#[test]
fn test_focus_cycling_bindings_exist() {
    let bindings = bindings();
    let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
    assert!(keys.contains(&"w"), "Should have 'w' for next window");
    assert!(keys.contains(&"W"), "Should have 'W' for previous window");
    assert!(keys.contains(&"p"), "Should have 'p' for previous window");
}

#[test]
fn test_new_window_binding_exists() {
    let bindings = bindings();
    assert!(bindings.iter().any(|b| b.keys == "n"), "Should have 'n' for new window");
}

#[test]
fn test_close_others_binding_exists() {
    let bindings = bindings();
    assert!(bindings.iter().any(|b| b.keys == "q"), "Should have 'q' for close window");
}

#[test]
fn test_float_zone_bindings_exist() {
    let bindings = bindings();
    let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
    assert!(keys.contains(&"f"), "Should have 'f' for toggle float");
    assert!(keys.contains(&"]"), "Should have ']' for raise float");
    assert!(keys.contains(&"["), "Should have '[' for lower float");
}

#[test]
fn test_escape_binding_exists() {
    let bindings = bindings();
    assert!(bindings.iter().any(|b| b.keys == "<Esc>"), "Should have '<Esc>' for escape");
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
        assert!(binding.category.is_some(), "Binding '{}' should have a category", binding.keys);
    }
}

#[test]
fn test_layer_opacity_bindings_exist() {
    let bindings = bindings();
    let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
    assert!(keys.contains(&"."), "Should have '.' for increase opacity");
    assert!(keys.contains(&","), "Should have ',' for decrease opacity");
}

#[test]
fn test_bindings_count() {
    let b = bindings();
    // Navigation (4) + Arrows (4) + Cycling (3) + Split (3) + Close (3) +
    // Resize (5) + Float (3) + Opacity (2) + Tabs (1) + Escape (1) = 29
    assert_eq!(b.len(), 29);
}
