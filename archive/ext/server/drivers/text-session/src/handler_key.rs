//! Session handler key - typed key for session handler lookup.
//!
//! Defines the typed key enum for session handler discovery.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for session handler lookup.
///
/// This enum defines the purposes for which session handlers can be registered.
/// Currently only `Empty` is supported for handling sessions without buffers.
///
/// # Compile-Time Safety
///
/// Using typed keys instead of strings ensures:
/// - Typos are caught at compile time (`SessionHandlerKey::Emtpy` → error)
/// - Exhaustive matching in `match` statements
/// - Self-documenting API
///
/// # Example
///
/// ```ignore
/// use reovim_driver_text_session::{SessionHandlerKey, SessionHandlerRegistry, EmptySessionHandler};
/// use std::sync::Arc;
///
/// let registry = SessionHandlerRegistry::new();
/// registry.register(SessionHandlerKey::Empty, Arc::new(ScratchBufferHandler::new()));
///
/// // Lookup with typed key
/// let handler = registry.get(&SessionHandlerKey::Empty);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionHandlerKey {
    /// Handler for empty sessions (no buffers).
    ///
    /// When a session starts with no files specified, this handler
    /// determines what to do (e.g., create a scratch buffer, show welcome).
    Empty,
}

impl ServiceKey for SessionHandlerKey {
    fn service_name() -> &'static str {
        "SessionHandler"
    }
}
#[cfg(test)]
#[path = "handler_key_tests.rs"]
mod tests;
