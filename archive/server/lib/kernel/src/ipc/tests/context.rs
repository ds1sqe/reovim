use super::*;

#[derive(Debug)]
struct TestEvent {
    #[allow(dead_code)]
    value: i32,
}
impl Event for TestEvent {}

// ========== HandlerContext tests ==========

#[test]
fn test_context_new() {
    let ctx = HandlerContext::new();
    assert!(!ctx.render_requested());
    assert!(!ctx.quit_requested());
    assert!(ctx.scope().is_none());
    assert!(!ctx.has_emitted_events());
}

#[test]
fn test_context_default() {
    let ctx = HandlerContext::default();
    assert!(!ctx.render_requested());
}

#[test]
fn test_context_request_render() {
    let mut ctx = HandlerContext::new();
    ctx.request_render();
    assert!(ctx.render_requested());
}

#[test]
fn test_context_request_quit() {
    let mut ctx = HandlerContext::new();
    ctx.request_quit();
    assert!(ctx.quit_requested());
}

#[test]
fn test_context_with_scope() {
    let scope = EventScope::new();
    let ctx = HandlerContext::new().with_scope(Some(scope.clone()));
    assert!(ctx.scope().is_some());
    assert_eq!(ctx.scope().unwrap().id(), scope.id());
}

#[test]
fn test_context_emit_without_scope() {
    let mut ctx = HandlerContext::new();
    ctx.emit(TestEvent { value: 42 });

    assert!(ctx.has_emitted_events());
    assert_eq!(ctx.emitted_event_count(), 1);

    let events = ctx.take_emitted_events();
    assert_eq!(events.len(), 1);
    assert!(events[0].scope().is_none());
}

#[test]
fn test_context_emit_with_scope() {
    let scope = EventScope::new();
    let mut ctx = HandlerContext::new().with_scope(Some(scope.clone()));

    assert_eq!(scope.in_flight(), 0);
    ctx.emit(TestEvent { value: 42 });
    assert_eq!(scope.in_flight(), 1);

    let events = ctx.take_emitted_events();
    assert_eq!(events.len(), 1);
    assert!(events[0].scope().is_some());
}

#[test]
fn test_context_emit_multiple() {
    let mut ctx = HandlerContext::new();
    ctx.emit(TestEvent { value: 1 });
    ctx.emit(TestEvent { value: 2 });
    ctx.emit(TestEvent { value: 3 });

    assert_eq!(ctx.emitted_event_count(), 3);
}

#[test]
fn test_context_take_emitted_clears() {
    let mut ctx = HandlerContext::new();
    ctx.emit(TestEvent { value: 42 });

    let _ = ctx.take_emitted_events();
    assert!(!ctx.has_emitted_events());
    assert_eq!(ctx.emitted_event_count(), 0);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_context_debug() {
    let ctx = HandlerContext::new();
    let debug_str = format!("{ctx:?}");
    assert!(debug_str.contains("HandlerContext"));
    assert!(debug_str.contains("render_requested"));
}

// ========== DispatchResult tests ==========

#[test]
fn test_dispatch_result_not_handled() {
    let result = DispatchResult::not_handled();
    assert_eq!(result.result, EventResult::NotHandled);
    assert!(!result.render_requested);
    assert!(!result.quit_requested);
    assert!(result.emitted_events.is_empty());
}

#[test]
fn test_dispatch_result_new() {
    let mut ctx = HandlerContext::new();
    ctx.request_render();
    ctx.emit(TestEvent { value: 42 });

    let result = DispatchResult::new(EventResult::Handled, &mut ctx);
    assert_eq!(result.result, EventResult::Handled);
    assert!(result.render_requested);
    assert!(!result.quit_requested);
    assert_eq!(result.emitted_events.len(), 1);
}

// ========== emit_dyn tests ==========

#[test]
fn test_context_emit_dyn_collect_mode() {
    let mut ctx = HandlerContext::new();
    let dyn_event = DynEvent::new(TestEvent { value: 99 });
    ctx.emit_dyn(dyn_event);

    assert!(ctx.has_emitted_events());
    assert_eq!(ctx.emitted_event_count(), 1);
    let events = ctx.take_emitted_events();
    assert_eq!(events.len(), 1);
}

#[test]
fn test_context_emit_dyn_direct_mode() {
    let bus = EventBus::new_with_channel(16);
    let tx = bus.sender().unwrap();
    let rx = bus.take_receiver().unwrap();

    let mut ctx = HandlerContext::new().with_sender(&tx);
    let dyn_event = DynEvent::new(TestEvent { value: 77 });
    ctx.emit_dyn(dyn_event);

    // In direct mode, emitted_events stays empty
    assert!(!ctx.has_emitted_events());
    // Event was sent via channel
    let evt = rx.try_recv().unwrap();
    assert_eq!(evt.downcast_ref::<TestEvent>().unwrap().value, 77);
}

// ========== emit with sender (direct mode) ==========

#[test]
fn test_context_emit_with_sender() {
    let bus = EventBus::new_with_channel(16);
    let tx = bus.sender().unwrap();
    let rx = bus.take_receiver().unwrap();

    let mut ctx = HandlerContext::new().with_sender(&tx);
    assert!(ctx.has_sender());

    ctx.emit(TestEvent { value: 55 });

    // Collect mode empty, sent via channel
    assert!(!ctx.has_emitted_events());
    let evt = rx.try_recv().unwrap();
    assert_eq!(evt.downcast_ref::<TestEvent>().unwrap().value, 55);
}

// ========== with_sender / has_sender ==========

#[test]
fn test_context_has_sender_false_by_default() {
    let ctx = HandlerContext::new();
    assert!(!ctx.has_sender());
}

#[test]
fn test_context_emit_with_scope_and_sender() {
    let bus = EventBus::new_with_channel(16);
    let tx = bus.sender().unwrap();
    let rx = bus.take_receiver().unwrap();
    let scope = EventScope::new();

    let mut ctx = HandlerContext::new()
        .with_sender(&tx)
        .with_scope(Some(scope.clone()));

    assert_eq!(scope.in_flight(), 0);
    ctx.emit(TestEvent { value: 33 });
    assert_eq!(scope.in_flight(), 1);

    let evt = rx.try_recv().unwrap();
    assert!(evt.scope().is_some());
}
