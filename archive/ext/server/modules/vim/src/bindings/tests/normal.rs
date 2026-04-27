use super::super::normal::*;

#[test]
fn test_bindings_not_empty() {
    let b = bindings();
    assert!(!b.is_empty());
}

#[test]
fn test_all_bindings_in_normal_mode() {
    for binding in bindings() {
        assert!(
            binding.modes.contains(&"vim:normal"),
            "Binding '{}' should be in vim:normal mode",
            binding.keys
        );
    }
}

#[test]
fn test_movement_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"h"), "Missing 'h' binding");
    assert!(keys.contains(&"j"), "Missing 'j' binding");
    assert!(keys.contains(&"k"), "Missing 'k' binding");
    assert!(keys.contains(&"l"), "Missing 'l' binding");
}

#[test]
fn test_word_motions_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"w"), "Missing 'w' binding");
    assert!(keys.contains(&"b"), "Missing 'b' binding");
    assert!(keys.contains(&"e"), "Missing 'e' binding");
    assert!(keys.contains(&"W"), "Missing 'W' binding");
    assert!(keys.contains(&"B"), "Missing 'B' binding");
    assert!(keys.contains(&"E"), "Missing 'E' binding");
}

#[test]
fn test_line_motions_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"0"), "Missing '0' binding");
    assert!(keys.contains(&"$"), "Missing '$' binding");
    assert!(keys.contains(&"^"), "Missing '^' binding");
}

#[test]
fn test_document_motions_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"gg"), "Missing 'gg' binding");
    assert!(keys.contains(&"G"), "Missing 'G' binding");
}

#[test]
fn test_mode_switching_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"i"), "Missing 'i' binding");
    assert!(keys.contains(&"a"), "Missing 'a' binding");
    assert!(keys.contains(&"A"), "Missing 'A' binding");
    assert!(keys.contains(&"I"), "Missing 'I' binding");
    assert!(keys.contains(&"o"), "Missing 'o' binding");
    assert!(keys.contains(&"O"), "Missing 'O' binding");
    assert!(keys.contains(&"v"), "Missing 'v' binding");
    assert!(keys.contains(&"V"), "Missing 'V' binding");
    assert!(keys.contains(&":"), "Missing ':' binding");
}

#[test]
fn test_operator_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"d"), "Missing 'd' binding");
    assert!(keys.contains(&"y"), "Missing 'y' binding");
    assert!(keys.contains(&"c"), "Missing 'c' binding");
}

#[test]
fn test_line_operator_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"dd"), "Missing 'dd' binding");
    assert!(keys.contains(&"yy"), "Missing 'yy' binding");
    assert!(keys.contains(&"cc"), "Missing 'cc' binding");
    assert!(keys.contains(&"D"), "Missing 'D' binding");
    assert!(keys.contains(&"C"), "Missing 'C' binding");
    assert!(keys.contains(&"Y"), "Missing 'Y' binding");
}

#[test]
fn test_single_char_edit_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"x"), "Missing 'x' binding");
    assert!(keys.contains(&"X"), "Missing 'X' binding");
    assert!(keys.contains(&"r"), "Missing 'r' binding");
    assert!(keys.contains(&"~"), "Missing '~' binding");
    assert!(keys.contains(&"."), "Missing '.' binding");
}

#[test]
fn test_undo_redo_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"u"), "Missing 'u' binding");
    assert!(keys.contains(&"<C-r>"), "Missing '<C-r>' binding");
}

#[test]
fn test_paste_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"p"), "Missing 'p' binding");
    assert!(keys.contains(&"P"), "Missing 'P' binding");
}

#[test]
fn test_scroll_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"<C-u>"), "Missing '<C-u>' binding");
    assert!(keys.contains(&"<C-d>"), "Missing '<C-d>' binding");
    assert!(keys.contains(&"<C-b>"), "Missing '<C-b>' binding");
    assert!(keys.contains(&"<C-f>"), "Missing '<C-f>' binding");
    assert!(keys.contains(&"zz"), "Missing 'zz' binding");
    assert!(keys.contains(&"zt"), "Missing 'zt' binding");
    assert!(keys.contains(&"zb"), "Missing 'zb' binding");
}

#[test]
fn test_search_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"/"), "Missing '/' binding");
    assert!(keys.contains(&"?"), "Missing '?' binding");
    assert!(keys.contains(&"n"), "Missing 'n' binding");
    assert!(keys.contains(&"N"), "Missing 'N' binding");
    assert!(keys.contains(&"*"), "Missing '*' binding");
    assert!(keys.contains(&"#"), "Missing '#' binding");
}

#[test]
fn test_find_char_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"f"), "Missing 'f' binding");
    assert!(keys.contains(&"F"), "Missing 'F' binding");
    assert!(keys.contains(&"t"), "Missing 't' binding");
    assert!(keys.contains(&"T"), "Missing 'T' binding");
    assert!(keys.contains(&";"), "Missing ';' binding");
    assert!(keys.contains(&","), "Missing ',' binding");
}

#[test]
fn test_window_key_exists() {
    let b = bindings();
    assert!(b.iter().any(|kb| kb.keys == "<C-w>"), "Missing '<C-w>' binding");
}

#[test]
fn test_mark_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"m"), "Missing 'm' binding");
    assert!(keys.contains(&"'"), "Missing \"'\" binding");
    assert!(keys.contains(&"`"), "Missing '`' binding");
}

#[test]
fn test_join_key_exists() {
    let b = bindings();
    assert!(b.iter().any(|kb| kb.keys == "J"), "Missing 'J' binding");
}

#[test]
fn test_display_line_motions_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"gj"), "Missing 'gj' binding");
    assert!(keys.contains(&"gk"), "Missing 'gk' binding");
}

#[test]
fn test_backward_word_end_motions_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"ge"), "Missing 'ge' binding");
    assert!(keys.contains(&"gE"), "Missing 'gE' binding");
}

#[test]
fn test_session_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"<C-b>d"), "Missing '<C-b>d' binding");
    assert!(keys.contains(&"<C-b>s"), "Missing '<C-b>s' binding");
    assert!(keys.contains(&"<C-b>&"), "Missing '<C-b>&' binding");
}

#[test]
fn test_tab_keys_exist() {
    let b = bindings();
    let keys: Vec<_> = b.iter().map(|kb| kb.keys).collect();
    assert!(keys.contains(&"gt"), "Missing 'gt' binding");
    assert!(keys.contains(&"gT"), "Missing 'gT' binding");
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
