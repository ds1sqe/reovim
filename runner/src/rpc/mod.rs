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

use reovim_protocol::v1::{
    EDITOR_QUIT, EDITOR_RESIZE, INPUT_KEYS, SERVER_KILL, STATE_ASCII_ART, STATE_CURSOR,
    STATE_LAYER_INFO, STATE_MICROSCOPE, STATE_MODE, STATE_SCREEN, STATE_SELECTION, STATE_TELESCOPE,
    STATE_VISUAL_SNAPSHOT, STATE_WINDOWS,
};

/// Create a dispatcher with all default handlers registered.
#[must_use]
pub fn create_default_dispatcher() -> RpcDispatcher {
    let mut dispatcher = RpcDispatcher::new();

    // Input methods
    dispatcher.register(INPUT_KEYS, handlers::input_keys);

    // State methods (implemented)
    dispatcher.register(STATE_MODE, handlers::state_mode);
    dispatcher.register(STATE_CURSOR, handlers::state_cursor);

    // State methods (stubs)
    dispatcher.register(STATE_SELECTION, handlers::state_selection);
    dispatcher.register(STATE_SCREEN, handlers::state_screen);
    dispatcher.register(STATE_WINDOWS, handlers::state_windows);
    dispatcher.register(STATE_TELESCOPE, handlers::state_telescope);
    dispatcher.register(STATE_MICROSCOPE, handlers::state_microscope);
    dispatcher.register(STATE_VISUAL_SNAPSHOT, handlers::state_visual_snapshot);
    dispatcher.register(STATE_ASCII_ART, handlers::state_ascii_art);
    dispatcher.register(STATE_LAYER_INFO, handlers::state_layer_info);

    // Editor methods (stubs)
    dispatcher.register(EDITOR_RESIZE, handlers::editor_resize);
    dispatcher.register(EDITOR_QUIT, handlers::editor_quit);

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

        // Implemented handlers
        assert!(dispatcher.has_method(INPUT_KEYS));
        assert!(dispatcher.has_method(STATE_MODE));
        assert!(dispatcher.has_method(STATE_CURSOR));
        assert!(dispatcher.has_method(SERVER_KILL));

        // Stub handlers
        assert!(dispatcher.has_method(STATE_SELECTION));
        assert!(dispatcher.has_method(STATE_SCREEN));
        assert!(dispatcher.has_method(STATE_WINDOWS));
        assert!(dispatcher.has_method(EDITOR_RESIZE));
        assert!(dispatcher.has_method(EDITOR_QUIT));

        // 4 implemented + 10 stubs = 14 total
        assert_eq!(dispatcher.method_count(), 14);
    }
}
