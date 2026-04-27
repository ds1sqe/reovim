use reovim_kernel::api::v1::{CursorStyle, Mode};

use super::*;

#[test]
fn manager_mode_id() {
    assert_eq!(
        ManagerMode::MANAGER_ID,
        reovim_kernel::api::v1::ModeId::with_discriminant(crate::ids::MODULE, "MANAGER", 0)
    );
}

#[test]
fn manager_module() {
    assert_eq!(ManagerMode::module(), crate::ids::MODULE);
}

#[test]
fn manager_discriminant() {
    assert_eq!(ManagerMode::Manager.discriminant(), 0);
}

#[test]
fn manager_display_name() {
    assert_eq!(ManagerMode::Manager.display_name(), "MANAGER");
}

#[test]
fn manager_cursor_style() {
    assert_eq!(ManagerMode::Manager.cursor_style(), CursorStyle::Block);
}

#[test]
fn manager_no_char_input() {
    assert!(!ManagerMode::Manager.accepts_char_input());
}

#[test]
fn all_modes_not_empty() {
    assert_eq!(ManagerMode::ALL.len(), 1);
}
