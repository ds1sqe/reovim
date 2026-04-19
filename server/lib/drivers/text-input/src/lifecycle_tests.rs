use {
    crate::{ModeLifecycleHandler, NopLifecycleHandler},
    reovim_kernel::api::v1::{ModeId, ModuleId},
};

#[test]
fn test_nop_lifecycle_handler() {
    let mut handler = NopLifecycleHandler;
    let mode = ModeId::with_discriminant(ModuleId::new("test"), "INSERT", 1);

    handler.on_input_mode_enter(&mode);
    handler.on_input_mode_exit(&mode);
    handler.on_mode_change(&mode, &mode);
    handler.on_mode_exit(&mode);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_lifecycle_handler_object_safe() {
    fn _accepts_ref(_: &dyn ModeLifecycleHandler) {}
    fn _accepts_box(_: Box<dyn ModeLifecycleHandler>) {}
}

#[allow(clippy::struct_field_names)]
struct TestLifecycleHandler {
    enter_count: usize,
    exit_count: usize,
    change_count: usize,
    mode_exit_count: usize,
}

impl TestLifecycleHandler {
    fn new() -> Self {
        Self {
            enter_count: 0,
            exit_count: 0,
            change_count: 0,
            mode_exit_count: 0,
        }
    }
}

impl ModeLifecycleHandler for TestLifecycleHandler {
    fn on_input_mode_enter(&mut self, _mode: &ModeId) {
        self.enter_count += 1;
    }

    fn on_input_mode_exit(&mut self, _mode: &ModeId) {
        self.exit_count += 1;
    }

    fn on_mode_change(&mut self, _from: &ModeId, _to: &ModeId) {
        self.change_count += 1;
    }

    fn on_mode_exit(&mut self, _mode: &ModeId) {
        self.mode_exit_count += 1;
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_lifecycle_handler_implementation() {
    let mut handler = TestLifecycleHandler::new();
    let mode = ModeId::with_discriminant(ModuleId::new("test"), "INSERT", 1);

    assert_eq!(handler.enter_count, 0);
    handler.on_input_mode_enter(&mode);
    assert_eq!(handler.enter_count, 1);

    assert_eq!(handler.exit_count, 0);
    handler.on_input_mode_exit(&mode);
    assert_eq!(handler.exit_count, 1);

    assert_eq!(handler.change_count, 0);
    handler.on_mode_change(&mode, &mode);
    assert_eq!(handler.change_count, 1);

    assert_eq!(handler.mode_exit_count, 0);
    handler.on_mode_exit(&mode);
    assert_eq!(handler.mode_exit_count, 1);
}
