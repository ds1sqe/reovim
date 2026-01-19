//! Registry for empty session handlers.
//!
//! Collects handlers from modules and resolves which action to take
//! when a session is empty.

use std::sync::Arc;

use {
    reovim_driver_session::{EmptySessionAction, EmptySessionContext, EmptySessionHandler},
    tracing::debug,
};

/// Registry for empty session handlers.
///
/// Handlers are stored sorted by priority (lower first).
pub struct EmptySessionHandlerRegistry {
    handlers: Vec<Arc<dyn EmptySessionHandler>>,
}

impl EmptySessionHandlerRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Register a handler.
    ///
    /// Handlers are automatically sorted by priority.
    pub fn register(&mut self, handler: Arc<dyn EmptySessionHandler>) {
        self.handlers.push(handler);
        self.handlers.sort_by_key(|h| h.priority());
    }

    /// Resolve the action for an empty session.
    ///
    /// Calls handlers in priority order until one returns an action
    /// other than `None`. Returns `None` if all handlers defer.
    pub fn resolve(&self, ctx: &EmptySessionContext) -> Option<EmptySessionAction> {
        for handler in &self.handlers {
            let action = handler.handle(ctx);
            if let action @ EmptySessionAction::CreateBuffer { .. } = action {
                debug!(handler_id = handler.id(), "Empty session handler resolved");
                return Some(action);
            }
            // EmptySessionAction::None means defer to next handler
        }
        None
    }

    /// Get number of registered handlers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Check if registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

impl Default for EmptySessionHandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    struct TestHandler {
        id: &'static str,
        priority: u32,
        action: EmptySessionAction,
    }

    impl EmptySessionHandler for TestHandler {
        fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
            self.action.clone()
        }

        fn priority(&self) -> u32 {
            self.priority
        }

        fn id(&self) -> &'static str {
            self.id
        }

        fn description(&self) -> &'static str {
            "Test handler"
        }
    }

    #[test]
    fn test_priority_ordering() {
        let mut registry = EmptySessionHandlerRegistry::new();

        // Register in wrong order
        registry.register(Arc::new(TestHandler {
            id: "low",
            priority: 200,
            action: EmptySessionAction::None,
        }));
        registry.register(Arc::new(TestHandler {
            id: "high",
            priority: 50,
            action: EmptySessionAction::CreateBuffer {
                name: None,
                content: String::new(),
            },
        }));

        // High priority should be called first
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/"),
        };

        let action = registry.resolve(&ctx);
        assert!(matches!(action, Some(EmptySessionAction::CreateBuffer { .. })));
    }

    #[test]
    fn test_defer_to_next() {
        let mut registry = EmptySessionHandlerRegistry::new();

        registry.register(Arc::new(TestHandler {
            id: "defer",
            priority: 50,
            action: EmptySessionAction::None,
        }));
        registry.register(Arc::new(TestHandler {
            id: "handle",
            priority: 100,
            action: EmptySessionAction::CreateBuffer {
                name: Some("scratch".into()),
                content: String::new(),
            },
        }));

        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/"),
        };

        let action = registry.resolve(&ctx);
        assert!(matches!(action, Some(EmptySessionAction::CreateBuffer { name: Some(_), .. })));
    }

    #[test]
    fn test_all_handlers_defer() {
        let mut registry = EmptySessionHandlerRegistry::new();
        registry.register(Arc::new(TestHandler {
            id: "defer1",
            priority: 50,
            action: EmptySessionAction::None,
        }));
        registry.register(Arc::new(TestHandler {
            id: "defer2",
            priority: 100,
            action: EmptySessionAction::None,
        }));

        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/"),
        };

        // All handlers defer → returns None
        assert!(registry.resolve(&ctx).is_none());
    }

    #[test]
    fn test_empty_registry() {
        let registry = EmptySessionHandlerRegistry::new();
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/"),
        };

        // Empty registry → returns None (no panic)
        assert!(registry.resolve(&ctx).is_none());
    }
}
