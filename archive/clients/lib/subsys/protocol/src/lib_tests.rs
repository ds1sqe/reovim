use super::*;

// Smoke tests for lib.rs re-exports.
// Full suites live in dispatch_tests.rs and parse_tests.rs.

#[test]
fn lib_re_exports_dispatch_outcome() {
    let o: DispatchOutcome = DispatchOutcome::Handled;
    assert_eq!(o, DispatchOutcome::Handled);
}

#[test]
fn lib_re_exports_notification_dispatcher() {
    #[derive(Debug)]
    struct MockDispatcher;
    impl NotificationDispatcher for MockDispatcher {}
    let d: Box<dyn NotificationDispatcher> = Box::new(MockDispatcher);
    assert!(format!("{d:?}").contains("MockDispatcher"));
}
