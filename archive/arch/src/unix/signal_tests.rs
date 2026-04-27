use super::*;

#[test]
fn test_signal_handler_creation() {
    let _handler = UnixSignalHandler::new();
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_register_handlers() {
    let mut handler = UnixSignalHandler::new();

    handler.on_resize(Box::new(|_size| {
        // Resize callback
    }));

    handler.on_interrupt(Box::new(|| {
        // Interrupt callback
    }));

    handler.on_suspend(Box::new(|| {
        // Suspend callback
    }));

    // Handlers are registered (even if not actively used with crossterm)
}

#[test]
fn test_signal_handler_default() {
    let _handler = UnixSignalHandler::default();
    // Verify Default implementation works
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_register_handlers_replaces_previous() {
    let mut handler = UnixSignalHandler::new();

    // Register first set
    handler.on_resize(Box::new(|_size| {}));
    handler.on_interrupt(Box::new(|| {}));
    handler.on_suspend(Box::new(|| {}));

    // Register second set (replaces first)
    handler.on_resize(Box::new(|_size| {}));
    handler.on_interrupt(Box::new(|| {}));
    handler.on_suspend(Box::new(|| {}));
    // Should not panic - handlers are simply replaced
}
