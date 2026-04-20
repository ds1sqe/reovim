use std::{path::Path, sync::Arc};

use {
    super::*,
    crate::{EmptySessionAction, EmptySessionContext},
};

// Mock handler for testing
struct MockHandler;

impl EmptySessionHandler for MockHandler {
    fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
        EmptySessionAction::None
    }

    fn id(&self) -> &'static str {
        "mock:handler"
    }

    fn description(&self) -> &'static str {
        "Mock handler for testing"
    }
}

#[test]
fn test_registry_register_and_get() {
    let registry = SessionHandlerRegistry::new();

    let handler = Arc::new(MockHandler);
    registry.register(SessionHandlerKey::Empty, handler);

    let retrieved = registry.get(&SessionHandlerKey::Empty);
    assert!(retrieved.is_some());
}

#[test]
fn test_registry_handler_works() {
    let registry = SessionHandlerRegistry::new();
    registry.register(SessionHandlerKey::Empty, Arc::new(MockHandler));

    let handler = registry.get(&SessionHandlerKey::Empty).unwrap();
    let ctx = EmptySessionContext {
        session_id: 1,
        file_args: &[],
        cwd: Path::new("/tmp"),
    };
    let action = handler.handle(&ctx);
    assert!(matches!(action, EmptySessionAction::None));
}

#[test]
fn test_registry_get_nonexistent() {
    let registry = SessionHandlerRegistry::new();
    assert!(registry.get(&SessionHandlerKey::Empty).is_none());
}

#[test]
fn test_registry_handler_metadata() {
    let registry = SessionHandlerRegistry::new();
    registry.register(SessionHandlerKey::Empty, Arc::new(MockHandler));

    let handler = registry.get(&SessionHandlerKey::Empty).unwrap();
    assert_eq!(handler.id(), "mock:handler");
    assert_eq!(handler.description(), "Mock handler for testing");
    assert_eq!(handler.priority(), 100); // default priority
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_registry_replace_handler() {
    struct AltHandler;

    impl EmptySessionHandler for AltHandler {
        fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
            EmptySessionAction::CreateBuffer {
                name: Some("alt".to_string()),
                content: String::new(),
            }
        }
        fn id(&self) -> &'static str {
            "alt:handler"
        }
        fn description(&self) -> &'static str {
            "Alt handler"
        }
    }

    let registry = SessionHandlerRegistry::new();
    registry.register(SessionHandlerKey::Empty, Arc::new(MockHandler));
    registry.register(SessionHandlerKey::Empty, Arc::new(AltHandler));

    let handler = registry.get(&SessionHandlerKey::Empty).unwrap();
    assert_eq!(handler.id(), "alt:handler");
}

#[test]
fn test_registry_keys() {
    let registry = SessionHandlerRegistry::new();
    registry.register(SessionHandlerKey::Empty, Arc::new(MockHandler));

    let keys = registry.keys();
    assert_eq!(keys.len(), 1);
    assert!(keys.contains(&SessionHandlerKey::Empty));
}
