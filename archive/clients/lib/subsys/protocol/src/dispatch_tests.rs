use super::*;

// =============================================================================
// DispatchOutcome
// =============================================================================

#[test]
fn dispatch_outcome_variants_debug() {
    assert!(format!("{:?}", DispatchOutcome::Handled).contains("Handled"));
    assert!(format!("{:?}", DispatchOutcome::Ignored).contains("Ignored"));
    assert!(format!("{:?}", DispatchOutcome::Unknown).contains("Unknown"));
}

#[test]
fn dispatch_outcome_equality() {
    assert_eq!(DispatchOutcome::Handled, DispatchOutcome::Handled);
    assert_eq!(DispatchOutcome::Ignored, DispatchOutcome::Ignored);
    assert_eq!(DispatchOutcome::Unknown, DispatchOutcome::Unknown);
}

#[test]
fn dispatch_outcome_inequality() {
    assert_ne!(DispatchOutcome::Handled, DispatchOutcome::Ignored);
    assert_ne!(DispatchOutcome::Handled, DispatchOutcome::Unknown);
    assert_ne!(DispatchOutcome::Ignored, DispatchOutcome::Unknown);
}

#[test]
fn dispatch_outcome_copy() {
    let a = DispatchOutcome::Handled;
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// NotificationDispatcher — compile-time object safety
// =============================================================================

#[derive(Debug)]
struct StubDispatcher;

impl NotificationDispatcher for StubDispatcher {}

#[test]
fn notification_dispatcher_object_safety() {
    let d: Box<dyn NotificationDispatcher> = Box::new(StubDispatcher);
    assert!(format!("{d:?}").contains("StubDispatcher"));
}

#[test]
fn notification_dispatcher_debug() {
    let d = StubDispatcher;
    assert!(format!("{d:?}").contains("StubDispatcher"));
}
