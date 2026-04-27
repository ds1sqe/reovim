use {
    super::*,
    crate::{api::module::ModuleId, core::ModeId},
};

#[test]
fn test_snapshot_kernel_state() {
    let ctx = KernelContext::default();
    let snapshot = snapshot_kernel_state(&ctx);

    assert_eq!(snapshot.buffer_count, 0);
    assert!(snapshot.buffer_ids.is_empty());
    assert_eq!(snapshot.event_queue_len, 0);
}

#[test]
fn test_snapshot_mode_stack() {
    let mode_id = ModeId::new(ModuleId::new("editor"), "normal");
    let stack = ModeStack::new(mode_id);
    let snapshot = snapshot_mode_stack(&stack);

    assert_eq!(snapshot.current, "editor:normal");
    assert_eq!(snapshot.depth, 1);
    assert_eq!(snapshot.stack.len(), 1);
}
