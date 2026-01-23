//! Session handler registry.
//!
//! Type alias for the session handler registry, keyed by purpose.

use reovim_kernel::api::v1::MultiServiceRegistry;

use crate::{EmptySessionHandler, handler_key::SessionHandlerKey};

/// Registry for session handlers, keyed by purpose.
///
/// This is a type alias for `MultiServiceRegistry<SessionHandlerKey, dyn EmptySessionHandler>`.
/// Currently only `Empty` key is supported for empty session handlers.
///
/// # Architecture
///
/// Following the VFS pattern (mechanism/policy separation):
/// - **Mechanism (driver)**: This registry type + `EmptySessionHandler` trait
/// - **Policy (module)**: `ScratchBufferHandler` in `modules/scratch-buffer`
///
/// # Example
///
/// ```ignore
/// use reovim_driver_session::{SessionHandlerKey, SessionHandlerRegistry, EmptySessionHandler};
/// use std::sync::Arc;
///
/// // Create registry (typically done by runner)
/// let registry = SessionHandlerRegistry::new();
///
/// // Modules register their handlers during init
/// registry.register(SessionHandlerKey::Empty, Arc::new(scratch_buffer_handler));
///
/// // Runner queries with typed key
/// let handler = registry.get(&SessionHandlerKey::Empty);
/// ```
pub type SessionHandlerRegistry = MultiServiceRegistry<SessionHandlerKey, dyn EmptySessionHandler>;

#[cfg(test)]
mod tests {
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
}
