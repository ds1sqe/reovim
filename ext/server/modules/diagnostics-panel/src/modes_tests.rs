use reovim_kernel::api::v1::{CursorStyle, Mode};

use super::*;

#[test]
fn mode_module() {
    assert_eq!(DiagnosticsPanelMode::module(), ids::MODULE);
}

#[test]
fn mode_discriminant() {
    assert_eq!(DiagnosticsPanelMode::Panel.discriminant(), 0);
}

#[test]
fn mode_display_name() {
    assert_eq!(DiagnosticsPanelMode::Panel.display_name(), "DIAGNOSTICS");
}

#[test]
fn mode_cursor_style() {
    assert_eq!(DiagnosticsPanelMode::Panel.cursor_style(), CursorStyle::Block);
}

#[test]
fn mode_does_not_accept_char_input() {
    assert!(!DiagnosticsPanelMode::Panel.accepts_char_input());
}

#[test]
fn mode_has_no_selection() {
    assert!(!DiagnosticsPanelMode::Panel.has_selection());
}

#[test]
fn mode_no_inheritance() {
    assert!(DiagnosticsPanelMode::Panel.inherits_from().is_none());
}

#[test]
fn mode_is_not_entry() {
    assert!(!DiagnosticsPanelMode::Panel.is_entry());
}

#[test]
fn mode_id_matches_constant() {
    assert_eq!(DiagnosticsPanelMode::Panel.id(), DiagnosticsPanelMode::PANEL_ID);
}

#[test]
fn all_modes() {
    assert_eq!(DiagnosticsPanelMode::ALL.len(), 1);
    assert_eq!(DiagnosticsPanelMode::ALL[0], DiagnosticsPanelMode::Panel);
}

#[test]
fn mode_copy_clone() {
    let mode = DiagnosticsPanelMode::Panel;
    let copied = mode;
    assert_eq!(mode, copied);
}

#[test]
fn mode_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(DiagnosticsPanelMode::Panel);
    assert!(set.contains(&DiagnosticsPanelMode::Panel));
}

#[test]
fn mode_debug() {
    let debug = format!("{:?}", DiagnosticsPanelMode::Panel);
    assert!(debug.contains("Panel"));
}
