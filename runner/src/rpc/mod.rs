//! RPC dispatch system for the reovim server.
//!
//! This module provides the JSON-RPC dispatch infrastructure:
//! - [`RpcDispatcher`]: Routes requests to handlers
//! - [`RpcContext`]: Context passed to handlers
//! - [`HandlerFn`]: Function pointer type for handlers
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                     RpcDispatcher                        │
//! │  ├── "input/keys"  → input_keys handler                 │
//! │  ├── "state/mode"  → state_mode handler                 │
//! │  ├── "state/cursor"→ state_cursor handler               │
//! │  └── "server/kill" → server_kill handler                │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use runner::rpc::{RpcDispatcher, RpcContext, create_default_dispatcher};
//!
//! // Create dispatcher with all handlers
//! let dispatcher = create_default_dispatcher();
//!
//! // Dispatch a request
//! let response = dispatcher.dispatch(request, &context).await;
//! ```

mod dispatcher;
pub mod handlers;

pub use dispatcher::{HandlerFn, HandlerFuture, RpcContext, RpcDispatcher, RpcResult};

use reovim_protocol::v1::{INPUT_KEYS, SERVER_KILL, STATE_CURSOR, STATE_MODE};

/// Create a dispatcher with all default handlers registered.
#[must_use]
pub fn create_default_dispatcher() -> RpcDispatcher {
    let mut dispatcher = RpcDispatcher::new();

    // Input methods
    dispatcher.register(INPUT_KEYS, handlers::input_keys);

    // State methods
    dispatcher.register(STATE_MODE, handlers::state_mode);
    dispatcher.register(STATE_CURSOR, handlers::state_cursor);

    // Server methods
    dispatcher.register(SERVER_KILL, handlers::server_kill);

    dispatcher
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_dispatcher() {
        let dispatcher = create_default_dispatcher();

        assert!(dispatcher.has_method(INPUT_KEYS));
        assert!(dispatcher.has_method(STATE_MODE));
        assert!(dispatcher.has_method(STATE_CURSOR));
        assert!(dispatcher.has_method(SERVER_KILL));

        assert_eq!(dispatcher.method_count(), 4);
    }
}
