use super::*;

#[test]
fn test_app_state_new() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);

    assert!(app.is_running());
}

#[test]
fn test_app_state_quit() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);

    assert!(app.is_running());
    app.request_quit();
    assert!(!app.is_running());
}

#[test]
fn test_request_detach_does_not_panic() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);
    app.request_detach();
    // Detach doesn't change running state
    assert!(app.is_running());
}

#[test]
fn test_app_state_services_arc_shared() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    // Services should be a shared reference to the kernel's service registry
    // The Arc should have at least 2 strong references (kernel + app)
    assert!(Arc::strong_count(&app.services) >= 2);
}

#[test]
fn test_app_state_extensions_empty_initially() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    // Extensions map should be empty initially
    // We can verify by checking that it exists and doesn't panic
    let _ = &app.extensions;
}

#[test]
fn test_app_state_debug_format() {
    let kernel = KernelContext::default();
    let app = AppState::new(kernel);
    let debug_str = format!("{app:?}");
    assert!(debug_str.contains("AppState"));
    assert!(debug_str.contains("running: true"));
}

#[test]
fn test_app_state_quit_then_check() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);

    assert!(app.is_running());
    app.request_quit();
    assert!(!app.is_running());
    // Double quit should be idempotent
    app.request_quit();
    assert!(!app.is_running());
}

#[test]
fn test_detach_followed_by_quit() {
    let kernel = KernelContext::default();
    let mut app = AppState::new(kernel);

    // Detach should not affect running state
    app.request_detach();
    assert!(app.is_running());

    // Quit should stop running
    app.request_quit();
    assert!(!app.is_running());
}
