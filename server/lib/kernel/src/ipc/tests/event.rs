use super::*;

use std::any::TypeId;

#[derive(Debug, PartialEq)]
struct TestEvent {
    value: i32,
}

impl Event for TestEvent {
    fn priority(&self) -> u32 {
        50
    }
}

#[derive(Debug)]
struct HighPriorityEvent;

impl Event for HighPriorityEvent {
    fn priority(&self) -> u32 {
        0
    }

    fn batchable(&self) -> bool {
        true
    }
}

#[derive(Debug)]
struct DefaultEvent;
impl Event for DefaultEvent {}

#[derive(Debug)]
struct OtherEvent;
impl Event for OtherEvent {}

#[derive(Debug, PartialEq)]
struct TargetedTestEvent {
    target: &'static str,
    #[allow(dead_code)] // Used in test assertions via Debug
    value: i32,
}
impl Event for TargetedTestEvent {}
impl TargetedEvent for TargetedTestEvent {
    fn target(&self) -> &str {
        self.target
    }
}

// ========== Event trait tests ==========

#[test]
fn test_event_default_priority() {
    let event = DefaultEvent;
    assert_eq!(event.priority(), 100);
}

#[test]
fn test_event_custom_priority() {
    let event = TestEvent { value: 42 };
    assert_eq!(event.priority(), 50);
}

#[test]
fn test_event_high_priority() {
    let event = HighPriorityEvent;
    assert_eq!(event.priority(), 0);
}

#[test]
fn test_event_default_batchable() {
    let event = DefaultEvent;
    assert!(!event.batchable());
}

#[test]
fn test_event_custom_batchable() {
    let event = HighPriorityEvent;
    assert!(event.batchable());
}

// ========== TargetedEvent tests ==========

#[test]
fn test_targeted_event_target() {
    let event = TargetedTestEvent {
        target: "my_plugin",
        value: 42,
    };
    assert_eq!(event.target(), "my_plugin");
}

#[test]
fn test_targeted_event_is_also_event() {
    let event = TargetedTestEvent {
        target: "component",
        value: 100,
    };
    // TargetedEvent also implements Event
    assert_eq!(event.priority(), 100); // default priority
}

// ========== EventResult tests ==========

#[test]
fn test_event_result_is_consumed() {
    assert!(EventResult::Consumed.is_consumed());
    assert!(!EventResult::Handled.is_consumed());
    assert!(!EventResult::NotHandled.is_consumed());
}

#[test]
fn test_event_result_is_handled() {
    assert!(!EventResult::Consumed.is_handled());
    assert!(EventResult::Handled.is_handled());
    assert!(!EventResult::NotHandled.is_handled());
}

#[test]
fn test_event_result_is_not_handled() {
    assert!(!EventResult::Consumed.is_not_handled());
    assert!(!EventResult::Handled.is_not_handled());
    assert!(EventResult::NotHandled.is_not_handled());
}

#[test]
fn test_event_result_default() {
    assert_eq!(EventResult::default(), EventResult::NotHandled);
}

// ========== DynEvent tests ==========

#[test]
fn test_dyn_event_new() {
    let event = DynEvent::new(TestEvent { value: 42 });
    assert!(event.type_name().contains("TestEvent"));
    assert_eq!(event.priority(), 50);
    assert!(event.scope().is_none());
}

#[test]
fn test_dyn_event_type_id() {
    let event = DynEvent::new(TestEvent { value: 42 });
    assert_eq!(event.type_id(), TypeId::of::<TestEvent>());
}

#[test]
fn test_dyn_event_is() {
    let event = DynEvent::new(TestEvent { value: 42 });
    assert!(event.is::<TestEvent>());
    assert!(!event.is::<OtherEvent>());
}

#[test]
fn test_dyn_event_downcast_ref() {
    let event = DynEvent::new(TestEvent { value: 42 });
    let downcasted = event.downcast_ref::<TestEvent>();
    assert!(downcasted.is_some());
    assert_eq!(downcasted.unwrap().value, 42);
}

#[test]
fn test_dyn_event_downcast_ref_wrong_type() {
    let event = DynEvent::new(TestEvent { value: 42 });
    let downcasted = event.downcast_ref::<OtherEvent>();
    assert!(downcasted.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dyn_event_downcast_mut() {
    let mut event = DynEvent::new(TestEvent { value: 42 });
    if let Some(inner) = event.downcast_mut::<TestEvent>() {
        inner.value = 100;
    }
    assert_eq!(event.downcast_ref::<TestEvent>().unwrap().value, 100);
}

#[test]
fn test_dyn_event_downcast_mut_wrong_type() {
    let mut event = DynEvent::new(TestEvent { value: 42 });
    let downcasted = event.downcast_mut::<OtherEvent>();
    assert!(downcasted.is_none());
}

#[test]
fn test_dyn_event_into_inner() {
    let event = DynEvent::new(TestEvent { value: 42 });
    let inner = event.into_inner::<TestEvent>();
    assert!(inner.is_ok());
    assert_eq!(inner.unwrap(), TestEvent { value: 42 });
}

#[test]
fn test_dyn_event_into_inner_wrong_type() {
    let event = DynEvent::new(TestEvent { value: 42 });
    let result = event.into_inner::<OtherEvent>();
    assert!(result.is_err());
}

#[test]
fn test_dyn_event_with_scope() {
    let scope = EventScope::new();
    let event = DynEvent::new(TestEvent { value: 42 }).with_scope(scope.clone());
    assert!(event.scope().is_some());
    assert_eq!(event.scope().unwrap().id(), scope.id());
}

#[test]
fn test_dyn_event_take_scope() {
    let scope = EventScope::new();
    let mut event = DynEvent::new(TestEvent { value: 42 }).with_scope(scope);

    let taken = event.take_scope();
    assert!(taken.is_some());
    assert!(event.scope().is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_dyn_event_debug() {
    let event = DynEvent::new(TestEvent { value: 42 });
    let debug_str = format!("{event:?}");
    assert!(debug_str.contains("DynEvent"));
    assert!(debug_str.contains("TestEvent"));
    assert!(debug_str.contains("50"));
}

#[test]
fn test_dyn_event_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DynEvent>();
}

// === Coverage: CacheUpdated event ===

#[test]
fn test_cache_updated_priority() {
    let event = CacheUpdated {
        buffer_id: crate::mm::BufferId::new(),
        kind: CacheKind::Highlights,
    };
    assert_eq!(event.priority(), 50);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cache_updated_clone_debug() {
    let event = CacheUpdated {
        buffer_id: crate::mm::BufferId::new(),
        kind: CacheKind::Decorations,
    };
    let cloned = event.clone();
    assert_eq!(cloned.kind, CacheKind::Decorations);
    let debug = format!("{event:?}");
    assert!(debug.contains("CacheUpdated"));
}

// === Coverage: CacheKind all variants ===

#[test]
fn test_cache_kind_all_variants() {
    use std::collections::HashSet;

    assert_eq!(CacheKind::Highlights, CacheKind::Highlights);
    assert_ne!(CacheKind::Highlights, CacheKind::Decorations);
    assert_ne!(CacheKind::Highlights, CacheKind::Both);
    let mut set = HashSet::new();
    set.insert(CacheKind::Highlights);
    set.insert(CacheKind::Decorations);
    set.insert(CacheKind::Both);
    assert_eq!(set.len(), 3);
}

// === Coverage: into_inner error preserves event ===

#[test]
fn test_dyn_event_into_inner_error_preserves() {
    let event = DynEvent::new(TestEvent { value: 42 });
    let result = event.into_inner::<OtherEvent>();
    assert!(result.is_err());
    let original = result.unwrap_err();
    assert!(original.is::<TestEvent>());
}

// === Coverage: take_scope when no scope set ===

#[test]
fn test_dyn_event_take_scope_none() {
    let mut event = DynEvent::new(TestEvent { value: 1 });
    assert!(event.take_scope().is_none());
}

// === MC/DC: downcast_ref FALSE branch (type mismatch) ===

#[test]
fn test_dyn_event_downcast_ref_type_mismatch_returns_none() {
    #[derive(Debug)]
    struct EventA;
    impl Event for EventA {}

    #[derive(Debug)]
    struct EventB;
    impl Event for EventB {}

    let event = DynEvent::new(EventA);
    assert!(event.downcast_ref::<EventB>().is_none());
    assert!(event.downcast_ref::<EventA>().is_some());
}

// === MC/DC: downcast_mut TRUE and FALSE branches ===

#[test]
fn test_dyn_event_downcast_mut_type_mismatch_returns_none() {
    #[derive(Debug)]
    struct EventC {
        x: i32,
    }
    impl Event for EventC {}

    #[derive(Debug)]
    struct EventD;
    impl Event for EventD {}

    let mut event = DynEvent::new(EventC { x: 7 });
    assert!(event.downcast_mut::<EventD>().is_none());
    if let Some(inner) = event.downcast_mut::<EventC>() {
        inner.x = 99;
    }
    assert_eq!(event.downcast_ref::<EventC>().unwrap().x, 99);
}

// === MC/DC: into_inner FALSE branch ===

#[test]
fn test_dyn_event_into_inner_type_mismatch_returns_err() {
    #[derive(Debug, PartialEq)]
    struct EventE {
        val: u32,
    }
    impl Event for EventE {}

    #[derive(Debug)]
    struct EventF;
    impl Event for EventF {}

    let event = DynEvent::new(EventE { val: 55 });
    let result = event.into_inner::<EventF>();
    assert!(result.is_err());
    let recovered = result.unwrap_err();
    assert!(recovered.is::<EventE>());
}
