//! Tests for `ModeInfo`.

use {super::ModeInfo, reovim_kernel::api::v1::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TestMode;

impl Mode for TestMode {
    fn module() -> ModuleId {
        ModuleId::new("test")
    }

    fn discriminant(&self) -> u16 {
        0
    }

    fn id(&self) -> ModeId {
        ModeId::new(ModuleId::new("test"), "test")
    }

    fn display_name(&self) -> &'static str {
        "TEST"
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::Block
    }

    fn accepts_char_input(&self) -> bool {
        false
    }

    fn has_selection(&self) -> bool {
        false
    }

    fn inherits_from(&self) -> Option<Self> {
        None
    }

    fn is_entry(&self) -> bool {
        false
    }
}

#[test]
fn mode_info_from_mode_copies_runtime_metadata() {
    let mode = TestMode;
    let id = mode.id();
    let info = ModeInfo::from_mode(mode);
    assert_eq!(info.id, id);
    assert_eq!(info.display_name, "TEST");
    assert_eq!(info.cursor_style, CursorStyle::Block);
    assert!(!info.accepts_char_input);
    assert!(!info.has_selection);
    assert!(info.inherits_from.is_none());
    assert!(!info.is_entry);
}
