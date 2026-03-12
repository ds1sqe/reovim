use reovim_kernel::api::v1::ModuleId;

use super::*;

#[test]
fn new_and_mode() {
    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let config = JumpParentMode::new(mode.clone());
    assert_eq!(config.mode(), &mode);
}
