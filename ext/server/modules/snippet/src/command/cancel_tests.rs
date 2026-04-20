use reovim_kernel::api::v1::ModuleId;

use super::*;

fn test_return_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "return")
}

#[test]
fn test_cancel_id() {
    assert_eq!(CancelSnippet::new(test_return_mode()).id(), ids::CANCEL);
}

#[test]
fn test_cancel_description() {
    assert!(
        !CancelSnippet::new(test_return_mode())
            .description()
            .is_empty()
    );
}
