
use super::*;


// Verify trait object safety
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_executor_object_safe() {
    fn _accepts_ref(_: &dyn CommandExecutor) {}
    fn _accepts_box(_: Box<dyn CommandExecutor>) {}
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_api_object_safe() {
    fn _accepts_ref(_: &dyn CommandApi) {}
    fn _accepts_box(_: Box<dyn CommandApi>) {}
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_handle_object_safe() {
    fn _accepts_ref(_: &dyn CommandHandle) {}
    fn _accepts_box(_: Box<dyn CommandHandle>) {}
    fn _accepts_arc(_: Arc<dyn CommandHandle>) {}
}

#[test]
fn test_command_executor_impl() {
    use reovim_kernel::api::v1::ModuleId;

    struct TestExecutor;

    impl CommandExecutor for TestExecutor {
        fn get_handle(&self, id: &CommandId) -> Option<Arc<dyn CommandHandle>> {
            if id.name() == "known" {
                struct SuccessHandle;
                impl CommandHandle for SuccessHandle {
                    fn execute(
                        &self,
                        _runtime: &mut crate::SessionRuntime<'_>,
                        _ctx: &CommandContext,
                    ) -> CommandResult {
                        CommandResult::Success
                    }
                }
                Some(Arc::new(SuccessHandle))
            } else {
                None
            }
        }
    }

    let executor = TestExecutor;

    let known = CommandId::new(ModuleId::new("test"), "known");
    let unknown = CommandId::new(ModuleId::new("test"), "unknown");

    assert!(executor.get_handle(&known).is_some());
    assert!(executor.get_handle(&unknown).is_none());
}