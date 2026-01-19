//! Empty session handler wiring for modules.
//!
//! Provides functions to wire module empty session handlers to the registry.
//! This module follows the same pattern as `commands.rs` and `keybindings.rs`.
//!
//! NOTE: This wiring infrastructure is prepared for dynamic module loading (#265).
//! Currently, the defaults module provides handlers via a factory function.
//! When #265 is implemented, these functions will be used to wire handlers
//! from dynamically loaded modules.

// TODO(#265): Remove this when dynamic module loading uses these functions
#![allow(dead_code)]

use std::{fmt, sync::Arc};

use {
    reovim_driver_session::EmptySessionHandler,
    reovim_kernel::api::v1::{EmptySessionHandlerRegistration, ModuleId},
};

use crate::server::registry::EmptySessionHandlerRegistry;

/// Result of an empty session handler wiring operation.
pub type EmptySessionWiringResult = Result<EmptySessionWiringStats, EmptySessionWiringError>;

/// Statistics from an empty session handler wiring operation.
#[derive(Debug, Default, Clone)]
pub struct EmptySessionWiringStats {
    /// Number of handlers successfully wired.
    pub handlers_wired: usize,
}

impl EmptySessionWiringStats {
    /// Create empty stats.
    #[must_use]
    pub const fn new() -> Self {
        Self { handlers_wired: 0 }
    }
}

/// Error during empty session handler wiring.
#[derive(Debug, Clone)]
pub enum EmptySessionWiringError {
    /// Mismatch between registrations and handlers count.
    CountMismatch {
        /// Number of registrations provided.
        registrations: usize,
        /// Number of handlers provided.
        handlers: usize,
        /// The module providing the handlers.
        module: ModuleId,
    },
}

impl fmt::Display for EmptySessionWiringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CountMismatch {
                registrations,
                handlers,
                module,
            } => {
                write!(
                    f,
                    "empty session handler count mismatch from module '{}': {} registrations but {} handlers",
                    module.as_str(),
                    registrations,
                    handlers
                )
            }
        }
    }
}

impl std::error::Error for EmptySessionWiringError {}

/// Wire empty session handlers to the registry.
///
/// Takes registrations (descriptors) and actual handler instances,
/// matching them by position and registering with the registry.
///
/// # Arguments
///
/// * `module_id` - The ID of the module providing the handlers
/// * `registrations` - The handler registrations from the module
/// * `handlers` - The actual handler instances (trait objects)
/// * `registry` - The registry to wire handlers to
///
/// # Returns
///
/// - `Ok(EmptySessionWiringStats)` with the number of handlers wired
/// - `Err(EmptySessionWiringError)` if registration/handler counts don't match
///
/// # Errors
///
/// Returns `EmptySessionWiringError::CountMismatch` if the number of
/// registrations doesn't match the number of handlers. This prevents
/// silent data loss from `.zip()` truncation.
pub fn wire_empty_session_handlers(
    module_id: &ModuleId,
    registrations: &[EmptySessionHandlerRegistration],
    handlers: Vec<Arc<dyn EmptySessionHandler>>,
    registry: &mut EmptySessionHandlerRegistry,
) -> EmptySessionWiringResult {
    // Validate counts match (addresses Telemetry's concern about .zip() truncation)
    if registrations.len() != handlers.len() {
        return Err(EmptySessionWiringError::CountMismatch {
            registrations: registrations.len(),
            handlers: handlers.len(),
            module: module_id.clone(),
        });
    }

    let mut stats = EmptySessionWiringStats::new();

    for (reg, handler) in registrations.iter().zip(handlers) {
        tracing::debug!(
            module = %module_id,
            handler_id = %reg.id,
            priority = reg.priority,
            "wiring empty session handler"
        );
        registry.register(handler);
        stats.handlers_wired += 1;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use reovim_driver_session::{EmptySessionAction, EmptySessionContext};

    use super::*;

    /// Test handler implementation.
    struct TestHandler {
        id: &'static str,
        priority: u32,
    }

    impl EmptySessionHandler for TestHandler {
        fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
            EmptySessionAction::CreateBuffer {
                name: Some(self.id.to_string()),
                content: String::new(),
            }
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
    fn test_wire_empty_session_handlers_basic() {
        let mut registry = EmptySessionHandlerRegistry::new();
        let module_id = ModuleId::new("test-module");

        let registrations = vec![
            EmptySessionHandlerRegistration::new("test:handler")
                .with_priority(100)
                .with_description("Test handler"),
        ];

        let handlers: Vec<Arc<dyn EmptySessionHandler>> = vec![Arc::new(TestHandler {
            id: "test:handler",
            priority: 100,
        })];

        let result =
            wire_empty_session_handlers(&module_id, &registrations, handlers, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.handlers_wired, 1);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_wire_empty_session_handlers_multiple() {
        let mut registry = EmptySessionHandlerRegistry::new();
        let module_id = ModuleId::new("test-module");

        let registrations = vec![
            EmptySessionHandlerRegistration::new("test:first").with_priority(50),
            EmptySessionHandlerRegistration::new("test:second").with_priority(100),
        ];

        let handlers: Vec<Arc<dyn EmptySessionHandler>> = vec![
            Arc::new(TestHandler {
                id: "test:first",
                priority: 50,
            }),
            Arc::new(TestHandler {
                id: "test:second",
                priority: 100,
            }),
        ];

        let result =
            wire_empty_session_handlers(&module_id, &registrations, handlers, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.handlers_wired, 2);
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_wire_empty_session_handlers_count_mismatch_more_registrations() {
        let mut registry = EmptySessionHandlerRegistry::new();
        let module_id = ModuleId::new("test-module");

        // Two registrations but only one handler
        let registrations = vec![
            EmptySessionHandlerRegistration::new("test:first"),
            EmptySessionHandlerRegistration::new("test:second"),
        ];

        let handlers: Vec<Arc<dyn EmptySessionHandler>> = vec![Arc::new(TestHandler {
            id: "test:first",
            priority: 100,
        })];

        let result =
            wire_empty_session_handlers(&module_id, &registrations, handlers, &mut registry);

        assert!(result.is_err());
        let err = result.unwrap_err();

        // Use match since there's only one variant (irrefutable pattern)
        let EmptySessionWiringError::CountMismatch {
            registrations,
            handlers,
            ..
        } = err;
        assert_eq!(registrations, 2);
        assert_eq!(handlers, 1);
    }

    #[test]
    fn test_wire_empty_session_handlers_count_mismatch_more_handlers() {
        let mut registry = EmptySessionHandlerRegistry::new();
        let module_id = ModuleId::new("test-module");

        // One registration but two handlers
        let registrations = vec![EmptySessionHandlerRegistration::new("test:first")];

        let handlers: Vec<Arc<dyn EmptySessionHandler>> = vec![
            Arc::new(TestHandler {
                id: "test:first",
                priority: 50,
            }),
            Arc::new(TestHandler {
                id: "test:second",
                priority: 100,
            }),
        ];

        let result =
            wire_empty_session_handlers(&module_id, &registrations, handlers, &mut registry);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, EmptySessionWiringError::CountMismatch { .. }));
    }

    #[test]
    fn test_wire_empty_session_handlers_empty() {
        let mut registry = EmptySessionHandlerRegistry::new();
        let module_id = ModuleId::new("test-module");

        let registrations: Vec<EmptySessionHandlerRegistration> = vec![];
        let handlers: Vec<Arc<dyn EmptySessionHandler>> = vec![];

        let result =
            wire_empty_session_handlers(&module_id, &registrations, handlers, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.handlers_wired, 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_wiring_stats_new() {
        let stats = EmptySessionWiringStats::new();
        assert_eq!(stats.handlers_wired, 0);
    }

    #[test]
    fn test_wiring_error_display() {
        let err = EmptySessionWiringError::CountMismatch {
            registrations: 2,
            handlers: 1,
            module: ModuleId::new("test"),
        };
        let msg = err.to_string();
        assert!(msg.contains("count mismatch"));
        assert!(msg.contains("test"));
        assert!(msg.contains('2'));
        assert!(msg.contains('1'));
    }

    #[test]
    fn test_handlers_registered_in_priority_order() {
        let mut registry = EmptySessionHandlerRegistry::new();
        let module_id = ModuleId::new("test-module");

        // Register in reverse priority order
        let registrations = vec![
            EmptySessionHandlerRegistration::new("test:low").with_priority(200),
            EmptySessionHandlerRegistration::new("test:high").with_priority(50),
        ];

        let handlers: Vec<Arc<dyn EmptySessionHandler>> = vec![
            Arc::new(TestHandler {
                id: "test:low",
                priority: 200,
            }),
            Arc::new(TestHandler {
                id: "test:high",
                priority: 50,
            }),
        ];

        let result =
            wire_empty_session_handlers(&module_id, &registrations, handlers, &mut registry);

        assert!(result.is_ok());

        // Registry sorts by priority, so high priority (50) should be first
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/"),
        };

        // The resolve() should return the high priority handler's action first
        let action = registry.resolve(&ctx);
        assert!(
            matches!(action, Some(EmptySessionAction::CreateBuffer { name: Some(name), .. }) if name == "test:high")
        );
    }
}
