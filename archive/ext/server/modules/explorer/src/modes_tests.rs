use super::*;

#[test]
fn mode_module() {
    assert_eq!(ExplorerMode::module(), ids::MODULE);
}

#[test]
fn browse_discriminant() {
    assert_eq!(ExplorerMode::Browse.discriminant(), 0);
}

#[test]
fn input_discriminant() {
    assert_eq!(ExplorerMode::Input.discriminant(), 1);
}

#[test]
fn browse_display_name() {
    assert_eq!(ExplorerMode::Browse.display_name(), "EXPLORER");
}

#[test]
fn input_display_name() {
    assert_eq!(ExplorerMode::Input.display_name(), "EXPLORER_INPUT");
}

#[test]
fn browse_cursor_style() {
    assert_eq!(ExplorerMode::Browse.cursor_style(), CursorStyle::Block);
}

#[test]
fn input_cursor_style() {
    assert_eq!(ExplorerMode::Input.cursor_style(), CursorStyle::Bar);
}

#[test]
fn browse_does_not_accept_char_input() {
    assert!(!ExplorerMode::Browse.accepts_char_input());
}

#[test]
fn input_accepts_char_input() {
    assert!(ExplorerMode::Input.accepts_char_input());
}

#[test]
fn browse_has_no_selection() {
    assert!(!ExplorerMode::Browse.has_selection());
}

#[test]
fn input_has_no_selection() {
    assert!(!ExplorerMode::Input.has_selection());
}

#[test]
fn browse_no_inheritance() {
    assert!(ExplorerMode::Browse.inherits_from().is_none());
}

#[test]
fn input_no_inheritance() {
    assert!(ExplorerMode::Input.inherits_from().is_none());
}

#[test]
fn browse_is_not_entry() {
    assert!(!ExplorerMode::Browse.is_entry());
}

#[test]
fn input_is_not_entry() {
    assert!(!ExplorerMode::Input.is_entry());
}

#[test]
fn browse_mode_id_matches_constant() {
    assert_eq!(ExplorerMode::Browse.id(), ExplorerMode::BROWSE_ID);
}

#[test]
fn input_mode_id_matches_constant() {
    assert_eq!(ExplorerMode::Input.id(), ExplorerMode::INPUT_ID);
}

#[test]
fn all_modes() {
    assert_eq!(ExplorerMode::ALL.len(), 2);
    assert_eq!(ExplorerMode::ALL[0], ExplorerMode::Browse);
    assert_eq!(ExplorerMode::ALL[1], ExplorerMode::Input);
}

#[test]
fn mode_copy_clone() {
    let mode = ExplorerMode::Browse;
    let copied = mode;
    assert_eq!(mode, copied);
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn mode_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(ExplorerMode::Browse);
    set.insert(ExplorerMode::Input);
    assert!(set.contains(&ExplorerMode::Browse));
    assert!(set.contains(&ExplorerMode::Input));
    assert_eq!(set.len(), 2);
}

#[test]
fn mode_debug() {
    let debug = format!("{:?}", ExplorerMode::Browse);
    assert!(debug.contains("Browse"));
    let debug = format!("{:?}", ExplorerMode::Input);
    assert!(debug.contains("Input"));
}
