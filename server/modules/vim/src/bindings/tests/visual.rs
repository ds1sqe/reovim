use super::super::visual::*;

#[test]
fn test_bindings_not_empty() {
    let b = bindings();
    assert!(!b.is_empty());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_all_bindings_in_visual_modes() {
    for binding in bindings() {
        let in_visual = binding.modes.contains(&"vim:visual")
            || binding.modes.contains(&"vim:visual-line")
            || binding.modes.contains(&"vim:visual-block");
        assert!(
            in_visual,
            "Binding '{}' should be in a visual mode, got {:?}",
            binding.keys, binding.modes
        );
    }
}

#[test]
fn test_exit_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"<Esc>"), "Missing '<Esc>' binding");
    assert!(keys.contains(&"<C-c>"), "Missing '<C-c>' binding");
}

#[test]
fn test_movement_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"h"), "Missing 'h' binding");
    assert!(keys.contains(&"j"), "Missing 'j' binding");
    assert!(keys.contains(&"k"), "Missing 'k' binding");
    assert!(keys.contains(&"l"), "Missing 'l' binding");
    assert!(keys.contains(&"w"), "Missing 'w' binding");
    assert!(keys.contains(&"b"), "Missing 'b' binding");
    assert!(keys.contains(&"e"), "Missing 'e' binding");
}

#[test]
fn test_line_motions_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"0"), "Missing '0' binding");
    assert!(keys.contains(&"$"), "Missing '$' binding");
}

#[test]
fn test_document_motions_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"gg"), "Missing 'gg' binding");
    assert!(keys.contains(&"G"), "Missing 'G' binding");
}

#[test]
fn test_selection_operation_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"o"), "Missing 'o' (swap anchor) binding");
    assert!(keys.contains(&"O"), "Missing 'O' (swap corner) binding");
}

#[test]
fn test_mode_toggle_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"v"), "Missing 'v' binding");
    assert!(keys.contains(&"V"), "Missing 'V' binding");
    assert!(keys.contains(&"<C-v>"), "Missing '<C-v>' binding");
}

#[test]
fn test_operator_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"d"), "Missing 'd' binding");
    assert!(keys.contains(&"y"), "Missing 'y' binding");
    assert!(keys.contains(&"c"), "Missing 'c' binding");
    assert!(keys.contains(&"x"), "Missing 'x' binding");
    assert!(keys.contains(&"<gt>"), "Missing '<gt>' binding");
    assert!(keys.contains(&"<lt>"), "Missing '<lt>' binding");
}

#[test]
fn test_case_operator_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"~"), "Missing '~' binding");
    assert!(keys.contains(&"u"), "Missing 'u' binding");
    assert!(keys.contains(&"U"), "Missing 'U' binding");
}

#[test]
fn test_join_key_exists() {
    let b = bindings();
    assert!(b.iter().any(|kb| kb.keys == "J"), "Missing 'J' binding");
}

#[test]
fn test_command_mode_key_exists() {
    let b = bindings();
    assert!(b.iter().any(|kb| kb.keys == ":"), "Missing ':' binding");
}

#[test]
fn test_inner_text_object_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"iw"), "Missing 'iw' binding");
    assert!(keys.contains(&"iW"), "Missing 'iW' binding");
    assert!(keys.contains(&"i\""), "Missing 'i\"' binding");
    assert!(keys.contains(&"i'"), "Missing \"i'\" binding");
    assert!(keys.contains(&"i("), "Missing 'i(' binding");
    assert!(keys.contains(&"i)"), "Missing 'i)' binding");
    assert!(keys.contains(&"ib"), "Missing 'ib' binding");
    assert!(keys.contains(&"i["), "Missing 'i[' binding");
    assert!(keys.contains(&"i]"), "Missing 'i]' binding");
    assert!(keys.contains(&"i{"), "Missing 'i{{' binding");
    assert!(keys.contains(&"i}"), "Missing 'i}}' binding");
    assert!(keys.contains(&"iB"), "Missing 'iB' binding");
    assert!(keys.contains(&"i<lt>"), "Missing 'i<lt>' binding");
    assert!(keys.contains(&"i>"), "Missing 'i>' binding");
}

#[test]
fn test_around_text_object_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"aw"), "Missing 'aw' binding");
    assert!(keys.contains(&"aW"), "Missing 'aW' binding");
    assert!(keys.contains(&"a\""), "Missing 'a\"' binding");
    assert!(keys.contains(&"a'"), "Missing \"a'\" binding");
    assert!(keys.contains(&"a("), "Missing 'a(' binding");
    assert!(keys.contains(&"a)"), "Missing 'a)' binding");
    assert!(keys.contains(&"ab"), "Missing 'ab' binding");
    assert!(keys.contains(&"a["), "Missing 'a[' binding");
    assert!(keys.contains(&"a]"), "Missing 'a]' binding");
    assert!(keys.contains(&"a{"), "Missing 'a{{' binding");
    assert!(keys.contains(&"a}"), "Missing 'a}}' binding");
    assert!(keys.contains(&"aB"), "Missing 'aB' binding");
    assert!(keys.contains(&"a<lt>"), "Missing 'a<lt>' binding");
    assert!(keys.contains(&"a>"), "Missing 'a>' binding");
}

#[test]
fn test_visual_insert_keys_exist() {
    let b = bindings();
    // I and A should be present (multiple times for different modes)
    assert!(b.iter().any(|kb| kb.keys == "I"), "Missing 'I' binding for visual insert");

    assert!(b.iter().any(|kb| kb.keys == "A"), "Missing 'A' binding for visual insert");
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
