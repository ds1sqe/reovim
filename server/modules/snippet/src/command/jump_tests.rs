use reovim_kernel::api::v1::ModuleId;

use super::*;

fn test_return_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "return")
}

#[test]
fn test_jump_next_id() {
    assert_eq!(JumpNext::new(test_return_mode()).id(), ids::JUMP_NEXT);
}

#[test]
fn test_jump_next_description() {
    assert!(!JumpNext::new(test_return_mode()).description().is_empty());
}

#[test]
fn test_jump_prev_id() {
    assert_eq!(JumpPrev.id(), ids::JUMP_PREV);
}

#[test]
fn test_jump_prev_description() {
    assert!(!JumpPrev.description().is_empty());
}
