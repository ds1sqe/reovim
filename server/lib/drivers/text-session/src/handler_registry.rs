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
/// - **Policy (module)**: `ScratchBufferHandler` in `server/modules/scratch-buffer`
///
/// # Example
///
/// ```ignore
/// use reovim_driver_text_session::{SessionHandlerKey, SessionHandlerRegistry, EmptySessionHandler};
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
#[path = "handler_registry_tests.rs"]
mod tests;
