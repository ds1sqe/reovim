use {
    super::*,
    std::{
        sync::{atomic::{AtomicI32, AtomicU32, Ordering}, Arc},
        thread,
    },
};

use reovim_arch::sync::Mutex;

#[derive(Debug)]
struct TestEvent {
    value: i32,
}

impl Event for TestEvent {
    fn priority(&self) -> u32 {
        50
    }
}

#[derive(Debug)]
struct OtherEvent;
impl Event for OtherEvent {}

// ========== Basic tests ==========

#[test]
fn test_event_bus_new() {
    let bus = EventBus::new();
    assert_eq!(bus.total_handler_count(), 0);
    assert!(bus.queue_is_empty());
}

#[test]
fn test_event_bus_subscribe() {
    let bus = EventBus::new();
    let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);
    assert_eq!(bus.handler_count::<TestEvent>(), 1);
}

#[test]
fn test_event_bus_unsubscribe_on_drop() {
    let bus = EventBus::new();
    {
        let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);
        assert_eq!(bus.handler_count::<TestEvent>(), 1);
    }
    assert_eq!(bus.handler_count::<TestEvent>(), 0);
}

#[test]
fn test_event_bus_emit() {
    let bus = EventBus::new();
    let received = Arc::new(AtomicI32::new(0));
    let received2 = received.clone();

    let _sub = bus.subscribe::<TestEvent, _>(100, move |event| {
        received2.store(event.value, Ordering::SeqCst);
        EventResult::Handled
    });

    let result = bus.emit(TestEvent { value: 42 });
    assert!(result.is_handled());
    assert_eq!(received.load(Ordering::SeqCst), 42);
}

#[test]
fn test_event_bus_emit_no_handlers() {
    let bus = EventBus::new();
    let result = bus.emit(TestEvent { value: 42 });
    assert!(result.is_not_handled());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_event_bus_emit_wrong_type() {
    let bus = EventBus::new();
    let called = Arc::new(AtomicU32::new(0));
    let called2 = called.clone();

    let _sub = bus.subscribe::<TestEvent, _>(100, move |_| {
        called2.fetch_add(1, Ordering::SeqCst);
        EventResult::Handled
    });

    // Emit different event type
    bus.emit(OtherEvent);
    assert_eq!(called.load(Ordering::SeqCst), 0);
}

// ========== Priority ordering tests ==========

#[test]
fn test_event_bus_priority_ordering() {
    let bus = EventBus::new();
    let order = Arc::new(Mutex::new(Vec::new()));

    let order1 = order.clone();
    let _sub1 = bus.subscribe::<TestEvent, _>(200, move |_| {
        order1.lock().push(200);
        EventResult::Handled
    });

    let order2 = order.clone();
    let _sub2 = bus.subscribe::<TestEvent, _>(50, move |_| {
        order2.lock().push(50);
        EventResult::Handled
    });

    let order3 = order.clone();
    let _sub3 = bus.subscribe::<TestEvent, _>(100, move |_| {
        order3.lock().push(100);
        EventResult::Handled
    });

    bus.emit(TestEvent { value: 1 });

    let order_result = order.lock().clone();
    assert_eq!(order_result, vec![50, 100, 200]);
}

// ========== Consumed stops propagation ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_event_bus_consumed_stops_propagation() {
    let bus = EventBus::new();
    let call_count = Arc::new(AtomicU32::new(0));

    let count1 = call_count.clone();
    let _sub1 = bus.subscribe::<TestEvent, _>(50, move |_| {
        count1.fetch_add(1, Ordering::SeqCst);
        EventResult::Consumed // Stop propagation
    });

    let count2 = call_count.clone();
    let _sub2 = bus.subscribe::<TestEvent, _>(100, move |_| {
        count2.fetch_add(1, Ordering::SeqCst);
        EventResult::Handled
    });

    let result = bus.emit(TestEvent { value: 1 });
    assert!(result.is_consumed());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

// ========== Async queue tests ==========

#[test]
fn test_event_bus_emit_async() {
    let bus = EventBus::new();

    bus.emit_async(TestEvent { value: 1 });
    bus.emit_async(TestEvent { value: 2 });

    assert_eq!(bus.queue_len(), 2);
}

#[test]
fn test_event_bus_process_queue() {
    let bus = EventBus::new();
    let sum = Arc::new(AtomicI32::new(0));
    let sum2 = sum.clone();

    let _sub = bus.subscribe::<TestEvent, _>(100, move |event| {
        sum2.fetch_add(event.value, Ordering::SeqCst);
        EventResult::Handled
    });

    bus.emit_async(TestEvent { value: 10 });
    bus.emit_async(TestEvent { value: 20 });
    bus.emit_async(TestEvent { value: 30 });

    assert_eq!(sum.load(Ordering::SeqCst), 0); // Not processed yet

    let count = bus.process_queue();
    assert_eq!(count, 3);
    assert_eq!(sum.load(Ordering::SeqCst), 60);
    assert!(bus.queue_is_empty());
}

// ========== Scoped event tests ==========

#[test]
fn test_event_bus_emit_scoped() {
    let bus = EventBus::new();
    let scope = EventScope::new();

    scope.increment();
    assert_eq!(scope.in_flight(), 1);

    bus.emit_scoped(TestEvent { value: 1 }, &scope);
    assert_eq!(scope.in_flight(), 0);
}

#[test]
fn test_event_bus_emit_async_scoped() {
    let bus = EventBus::new();
    let scope = EventScope::new();

    bus.emit_async_scoped(TestEvent { value: 1 }, &scope);
    assert_eq!(scope.in_flight(), 1);

    let count = bus.process_queue();
    assert_eq!(count, 1);
    assert_eq!(scope.in_flight(), 0);
}

// ========== Multiple subscriptions ==========

#[test]
fn test_event_bus_multiple_subscriptions() {
    let bus = EventBus::new();
    let count = Arc::new(AtomicU32::new(0));

    let count1 = count.clone();
    let _sub1 = bus.subscribe::<TestEvent, _>(100, move |_| {
        count1.fetch_add(1, Ordering::SeqCst);
        EventResult::Handled
    });

    let count2 = count.clone();
    let _sub2 = bus.subscribe::<TestEvent, _>(100, move |_| {
        count2.fetch_add(1, Ordering::SeqCst);
        EventResult::Handled
    });

    bus.emit(TestEvent { value: 1 });
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn test_event_bus_partial_unsubscribe() {
    let bus = EventBus::new();
    let count = Arc::new(AtomicU32::new(0));

    let count1 = count.clone();
    let sub1 = bus.subscribe::<TestEvent, _>(100, move |_| {
        count1.fetch_add(1, Ordering::SeqCst);
        EventResult::Handled
    });

    let count2 = count.clone();
    let _sub2 = bus.subscribe::<TestEvent, _>(100, move |_| {
        count2.fetch_add(10, Ordering::SeqCst);
        EventResult::Handled
    });

    bus.emit(TestEvent { value: 1 });
    assert_eq!(count.load(Ordering::SeqCst), 11);

    drop(sub1);

    count.store(0, Ordering::SeqCst);
    bus.emit(TestEvent { value: 1 });
    assert_eq!(count.load(Ordering::SeqCst), 10);
}

// ========== Thread safety tests ==========

#[test]
fn test_event_bus_concurrent_emit() {
    let bus = Arc::new(EventBus::new());
    let count = Arc::new(AtomicU32::new(0));

    let count_handler = count.clone();
    let _sub = bus.subscribe::<TestEvent, _>(100, move |_| {
        count_handler.fetch_add(1, Ordering::SeqCst);
        EventResult::Handled
    });

    let mut handles = vec![];
    for i in 0..10 {
        let bus = bus.clone();
        handles.push(thread::spawn(move || {
            bus.emit(TestEvent { value: i });
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(count.load(Ordering::SeqCst), 10);
}

#[test]
fn test_event_bus_concurrent_subscribe_emit() {
    let bus = Arc::new(EventBus::new());

    let mut handles = vec![];

    // Subscribe from multiple threads
    for _ in 0..5 {
        let bus = bus.clone();
        handles.push(thread::spawn(move || {
            let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);
            // Keep subscription alive briefly
            thread::sleep(std::time::Duration::from_millis(10));
        }));
    }

    // Emit from multiple threads
    for i in 0..5 {
        let bus = bus.clone();
        handles.push(thread::spawn(move || {
            bus.emit(TestEvent { value: i });
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

// ========== Debug and Default tests ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_event_bus_debug() {
    let bus = EventBus::new();
    let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);
    let debug_str = format!("{bus:?}");
    assert!(debug_str.contains("EventBus"));
    assert!(debug_str.contains("total_handlers"));
}

#[test]
fn test_event_bus_default() {
    let bus = EventBus::default();
    assert_eq!(bus.total_handler_count(), 0);
}

#[test]
fn test_event_bus_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EventBus>();
}

// ========== Channel tests ==========

#[test]
fn test_event_bus_new_with_channel() {
    let bus = EventBus::new_with_channel(16);
    assert!(bus.sender().is_some());
    assert!(bus.take_receiver().is_some());
    // Second take returns None
    assert!(bus.take_receiver().is_none());
}

#[test]
fn test_event_bus_no_channel_no_sender() {
    let bus = EventBus::new();
    assert!(bus.sender().is_none());
    assert!(bus.take_receiver().is_none());
}

#[test]
fn test_event_sender_try_send() {
    let bus = EventBus::new_with_channel(16);
    let sender = bus.sender().unwrap();
    let receiver = bus.take_receiver().unwrap();

    sender.try_send(TestEvent { value: 42 });

    // Try receive
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.downcast_ref::<TestEvent>().unwrap().value, 42);
}

#[test]
fn test_event_sender_blocking_send() {
    let bus = EventBus::new_with_channel(16);
    let sender = bus.sender().unwrap();
    let receiver = bus.take_receiver().unwrap();

    // Use blocking send (won't block since channel has capacity)
    sender.send(TestEvent { value: 99 });

    let event = receiver.try_recv().unwrap();
    assert_eq!(event.downcast_ref::<TestEvent>().unwrap().value, 99);
}

#[test]
fn test_event_sender_send_scoped() {
    let bus = EventBus::new_with_channel(16);
    let sender = bus.sender().unwrap();
    let receiver = bus.take_receiver().unwrap();
    let scope = EventScope::new();

    assert_eq!(scope.in_flight(), 0);
    sender.send_scoped(TestEvent { value: 42 }, &scope);
    assert_eq!(scope.in_flight(), 1);

    let event = receiver.try_recv().unwrap();
    assert!(event.scope().is_some());
}

#[test]
fn test_event_sender_clone() {
    let bus = EventBus::new_with_channel(16);
    let sender1 = bus.sender().unwrap();
    let sender2 = sender1.clone();
    let receiver = bus.take_receiver().unwrap();

    sender1.try_send(TestEvent { value: 1 });
    sender2.try_send(TestEvent { value: 2 });

    let _ = receiver.try_recv().unwrap();
    let _ = receiver.try_recv().unwrap();
}

#[test]
fn test_event_sender_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EventSender>();
}

// ========== Context handler tests ==========

#[test]
fn test_subscribe_with_context() {
    let bus = EventBus::new();
    let render_requested = Arc::new(AtomicU32::new(0));
    let render_copy = render_requested.clone();

    let _sub = bus.subscribe_with_context::<TestEvent, _>(100, move |_event, ctx| {
        ctx.request_render();
        render_copy.fetch_add(1, Ordering::SeqCst);
        EventResult::Handled
    });

    let mut ctx = HandlerContext::new();
    let event = DynEvent::new(TestEvent { value: 42 });
    let result = bus.dispatch_with_context(&event, &mut ctx);

    assert_eq!(result.result, EventResult::Handled);
    assert!(result.render_requested);
    assert_eq!(render_requested.load(Ordering::SeqCst), 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_subscribe_with_context_emit() {
    #[derive(Debug)]
    struct FollowUpEvent;
    impl Event for FollowUpEvent {}

    let bus = EventBus::new();

    let _sub = bus.subscribe_with_context::<TestEvent, _>(100, move |_event, ctx| {
        ctx.emit(FollowUpEvent);
        EventResult::Handled
    });

    let mut ctx = HandlerContext::new();
    let event = DynEvent::new(TestEvent { value: 42 });
    let result = bus.dispatch_with_context(&event, &mut ctx);

    assert_eq!(result.emitted_events.len(), 1);
}

// ========== Targeted event tests ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_subscribe_targeted() {
    #[derive(Debug)]
    struct TargetedTestEvent {
        target: &'static str,
        #[allow(dead_code)]
        value: i32,
    }
    impl Event for TargetedTestEvent {}
    impl TargetedEvent for TargetedTestEvent {
        fn target(&self) -> &str {
            self.target
        }
    }

    let bus = EventBus::new();
    let called = Arc::new(AtomicU32::new(0));
    let called_copy = called.clone();

    let _sub = bus.subscribe_targeted::<TargetedTestEvent, _>(
        "my_plugin",
        100,
        move |_event, _ctx| {
            called_copy.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        },
    );

    // Event with matching target - should be handled
    let mut ctx = HandlerContext::new();
    let event = DynEvent::new(TargetedTestEvent {
        target: "my_plugin",
        value: 42,
    });
    let result = bus.dispatch_with_context(&event, &mut ctx);
    assert_eq!(result.result, EventResult::Handled);
    assert_eq!(called.load(Ordering::SeqCst), 1);

    // Event with different target - should NOT be handled
    let event2 = DynEvent::new(TargetedTestEvent {
        target: "other_plugin",
        value: 100,
    });
    let result2 = bus.dispatch_with_context(&event2, &mut ctx);
    assert_eq!(result2.result, EventResult::NotHandled);
    assert_eq!(called.load(Ordering::SeqCst), 1); // Count unchanged
}

// ========== dispatch_with_context Consumed result ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_dispatch_with_context_consumed() {
    let bus = EventBus::new();

    let _sub = bus.subscribe_with_context::<TestEvent, _>(50, move |_event, ctx| {
        ctx.request_quit();
        EventResult::Consumed
    });

    // Second handler should NOT be called after Consumed
    let second_called = Arc::new(AtomicU32::new(0));
    let second_called_copy = second_called.clone();
    let _sub2 = bus.subscribe_with_context::<TestEvent, _>(100, move |_event, _ctx| {
        second_called_copy.fetch_add(1, Ordering::SeqCst);
        EventResult::Handled
    });

    let mut ctx = HandlerContext::new();
    let event = DynEvent::new(TestEvent { value: 1 });
    let result = bus.dispatch_with_context(&event, &mut ctx);

    assert_eq!(result.result, EventResult::Consumed);
    assert!(result.quit_requested);
    assert_eq!(second_called.load(Ordering::SeqCst), 0);
}

// ========== dispatch_with_context not_handled ==========

#[test]
fn test_dispatch_with_context_no_handlers() {
    let bus = EventBus::new();
    let mut ctx = HandlerContext::new();
    let event = DynEvent::new(TestEvent { value: 1 });
    let result = bus.dispatch_with_context(&event, &mut ctx);

    assert_eq!(result.result, EventResult::NotHandled);
    assert!(!result.render_requested);
    assert!(!result.quit_requested);
    assert!(result.emitted_events.is_empty());
}

// ========== dispatch NotHandled from all handlers ==========

#[test]
fn test_dispatch_all_not_handled() {
    let bus = EventBus::new();

    let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::NotHandled);

    let result = bus.emit(TestEvent { value: 1 });
    assert!(result.is_not_handled());
}

// === Coverage: dispatch_with_context Simple handler returning NotHandled ===

#[test]
fn test_dispatch_with_context_simple_not_handled() {
    let bus = EventBus::new();
    let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::NotHandled);

    let mut ctx = HandlerContext::new();
    let event = DynEvent::new(TestEvent { value: 1 });
    let result = bus.dispatch_with_context(&event, &mut ctx);
    assert_eq!(result.result, EventResult::NotHandled);
}

// === Coverage: dispatch_with_context Simple handler returning Handled ===

#[test]
fn test_dispatch_with_context_simple_handled() {
    let bus = EventBus::new();
    let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);

    let mut ctx = HandlerContext::new();
    let event = DynEvent::new(TestEvent { value: 1 });
    let result = bus.dispatch_with_context(&event, &mut ctx);
    assert_eq!(result.result, EventResult::Handled);
}

// === Coverage: context handler returning NotHandled ===

#[test]
fn test_dispatch_with_context_all_not_handled() {
    let bus = EventBus::new();
    let _sub =
        bus.subscribe_with_context::<TestEvent, _>(100, |_event, _ctx| EventResult::NotHandled);

    let mut ctx = HandlerContext::new();
    let event = DynEvent::new(TestEvent { value: 1 });
    let result = bus.dispatch_with_context(&event, &mut ctx);
    assert_eq!(result.result, EventResult::NotHandled);
}

// === Coverage: EventBus clone ===

#[test]
fn test_event_bus_clone() {
    let bus = EventBus::new();
    let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);

    let bus2 = bus.clone();
    drop(bus);
    assert_eq!(bus2.handler_count::<TestEvent>(), 1);
}

// === Coverage: handler_count for unregistered type ===

#[test]
fn test_handler_count_no_handlers() {
    let bus = EventBus::new();
    assert_eq!(bus.handler_count::<OtherEvent>(), 0);
}

// === MC/DC: subscribe_targeted - FALSE branch (target mismatch) ===
// Exercises line 429 FALSE branch: event.target() != target
// Also covers the TRUE branch (matching target) to satisfy MC/DC.

#[test]
fn test_subscribe_targeted_false_branch_target_mismatch() {
    #[derive(Debug)]
    struct PluginEvent {
        target: &'static str,
    }
    impl Event for PluginEvent {}
    impl TargetedEvent for PluginEvent {
        fn target(&self) -> &str {
            self.target
        }
    }

    let bus = EventBus::new();
    let call_count = Arc::new(AtomicU32::new(0));
    let call_count2 = call_count.clone();

    let _sub =
        bus.subscribe_targeted::<PluginEvent, _>("correct_target", 100, move |_event, _ctx| {
            call_count2.fetch_add(1, Ordering::SeqCst);
            EventResult::Handled
        });

    // Emit with non-matching target: FALSE branch of `if event.target() == target`
    // Handler should NOT be called
    let wrong_target_event = DynEvent::new(PluginEvent {
        target: "wrong_target",
    });
    let mut ctx = HandlerContext::new();
    let result = bus.dispatch_with_context(&wrong_target_event, &mut ctx);
    assert_eq!(result.result, EventResult::NotHandled);
    assert_eq!(call_count.load(Ordering::SeqCst), 0);

    // Emit with matching target: TRUE branch of `if event.target() == target`
    // Handler SHOULD be called
    let correct_target_event = DynEvent::new(PluginEvent {
        target: "correct_target",
    });
    let mut ctx2 = HandlerContext::new();
    let result2 = bus.dispatch_with_context(&correct_target_event, &mut ctx2);
    assert_eq!(result2.result, EventResult::Handled);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

// === MC/DC: process_queue - scope present branch ===
// Exercises line 560 TRUE branch: scope is Some, decrement is called

#[test]
fn test_process_queue_with_scope_decrements() {
    let bus = EventBus::new();
    let scope = EventScope::new();

    // emit_async_scoped increments the scope counter
    bus.emit_async_scoped(TestEvent { value: 1 }, &scope);
    bus.emit_async_scoped(TestEvent { value: 2 }, &scope);
    assert_eq!(scope.in_flight(), 2);

    // process_queue should decrement the scope for each event (TRUE branch at line 560)
    let count = bus.process_queue();
    assert_eq!(count, 2);
    assert_eq!(scope.in_flight(), 0);
}

// === MC/DC: dispatch - no handlers registered for event type ===
// Exercises line 577 early return when handlers map has no entry for type_id

#[test]
fn test_dispatch_returns_not_handled_when_no_handlers_for_type() {
    let bus = EventBus::new();

    // Register a handler for TestEvent (so the handlers map is non-empty)
    let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);

    // Dispatch OtherEvent which has NO handlers: early return at line 577
    let dyn_event = DynEvent::new(OtherEvent);
    let result = bus.dispatch(&dyn_event);
    assert_eq!(result, EventResult::NotHandled);
}

// === MC/DC: dispatch_with_context - no handlers for event type ===
// Exercises line 637 early return: no handlers in the map for the event's TypeId

#[test]
fn test_dispatch_with_context_no_handlers_for_type() {
    let bus = EventBus::new();

    // Register a handler for TestEvent (so map is non-empty overall)
    let _sub = bus.subscribe::<TestEvent, _>(100, |_| EventResult::Handled);

    // Dispatch OtherEvent (no handlers): hits the early return at line 637
    let dyn_event = DynEvent::new(OtherEvent);
    let mut ctx = HandlerContext::new();
    let result = bus.dispatch_with_context(&dyn_event, &mut ctx);
    assert_eq!(result.result, EventResult::NotHandled);
    assert!(!result.render_requested);
}
