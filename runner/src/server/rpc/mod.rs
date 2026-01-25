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
    BUFFER_GET_CONTENT, BUFFER_LIST, BUFFER_OPEN_FILE, BUFFER_SET_CONTENT, BUFFER_WRITE_FILE,
    COMMAND_EXECUTE, EDITOR_QUIT, EDITOR_RESIZE, EDITOR_SET_ACTIVE_BUFFER, INPUT_KEYS, MODULE_LIST,
    MODULE_LOAD, MODULE_RELOAD, MODULE_UNLOAD, SERVER_KILL, STATE_ASCII_ART, STATE_CURSOR,
    STATE_LAYER_INFO, STATE_LAYOUT, STATE_MICROSCOPE, STATE_MODE, STATE_SCREEN,
    STATE_SCREEN_CONTENT, STATE_SELECTION, STATE_TELESCOPE, STATE_VISUAL_SNAPSHOT, STATE_WINDOWS,
};

use super::debug;

/// Create a dispatcher with all default handlers registered.
#[must_use]
pub fn create_default_dispatcher() -> RpcDispatcher {
    let mut dispatcher = RpcDispatcher::new();

    // Input methods
    dispatcher.register(INPUT_KEYS, handlers::input_keys);

    // Command methods
    dispatcher.register(COMMAND_EXECUTE, handlers::command_execute);

    // Buffer methods
    dispatcher.register(BUFFER_GET_CONTENT, handlers::buffer_get_content);
    dispatcher.register(BUFFER_SET_CONTENT, handlers::buffer_set_content);
    dispatcher.register(BUFFER_LIST, handlers::buffer_list);
    dispatcher.register(BUFFER_OPEN_FILE, handlers::buffer_open_file);
    dispatcher.register(BUFFER_WRITE_FILE, handlers::buffer_write_file);

    // State methods (implemented)
    dispatcher.register(STATE_MODE, handlers::state_mode);
    dispatcher.register(STATE_CURSOR, handlers::state_cursor);
    dispatcher.register(STATE_SCREEN_CONTENT, handlers::state_screen_content);

    // State methods (implemented)
    dispatcher.register(STATE_LAYOUT, handlers::state_layout);

    // State methods (stubs)
    dispatcher.register(STATE_SELECTION, handlers::state_selection);
    dispatcher.register(STATE_SCREEN, handlers::state_screen);
    dispatcher.register(STATE_WINDOWS, handlers::state_windows);
    dispatcher.register(STATE_TELESCOPE, handlers::state_telescope);
    dispatcher.register(STATE_MICROSCOPE, handlers::state_microscope);
    dispatcher.register(STATE_VISUAL_SNAPSHOT, handlers::state_visual_snapshot);
    dispatcher.register(STATE_ASCII_ART, handlers::state_ascii_art);
    dispatcher.register(STATE_LAYER_INFO, handlers::state_layer_info);

    // Editor methods
    dispatcher.register(EDITOR_RESIZE, handlers::editor_resize);
    dispatcher.register(EDITOR_QUIT, handlers::editor_quit);
    dispatcher.register(EDITOR_SET_ACTIVE_BUFFER, handlers::editor_set_active_buffer);

    // Module methods
    dispatcher.register(MODULE_LIST, handlers::module_list);
    dispatcher.register(MODULE_LOAD, handlers::module_load);
    dispatcher.register(MODULE_UNLOAD, handlers::module_unload);
    dispatcher.register(MODULE_RELOAD, handlers::module_reload);

    // Server methods
    dispatcher.register(SERVER_KILL, handlers::server_kill);

    // Debug methods
    debug::register_handlers(&mut dispatcher);

    dispatcher
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_dispatcher() {
        let dispatcher = create_default_dispatcher();

        // Input handlers
        assert!(dispatcher.has_method(INPUT_KEYS));

        // Command handlers
        assert!(dispatcher.has_method(COMMAND_EXECUTE));

        // Buffer handlers
        assert!(dispatcher.has_method(BUFFER_GET_CONTENT));
        assert!(dispatcher.has_method(BUFFER_SET_CONTENT));
        assert!(dispatcher.has_method(BUFFER_LIST));
        assert!(dispatcher.has_method(BUFFER_OPEN_FILE));
        assert!(dispatcher.has_method(BUFFER_WRITE_FILE));

        // State handlers (implemented)
        assert!(dispatcher.has_method(STATE_MODE));
        assert!(dispatcher.has_method(STATE_CURSOR));
        assert!(dispatcher.has_method(STATE_SCREEN_CONTENT));

        // State handlers (stubs)
        assert!(dispatcher.has_method(STATE_SELECTION));
        assert!(dispatcher.has_method(STATE_SCREEN));
        assert!(dispatcher.has_method(STATE_WINDOWS));

        // Module handlers
        assert!(dispatcher.has_method(MODULE_LIST));
        assert!(dispatcher.has_method(MODULE_LOAD));
        assert!(dispatcher.has_method(MODULE_UNLOAD));
        assert!(dispatcher.has_method(MODULE_RELOAD));

        // Editor handlers
        assert!(dispatcher.has_method(EDITOR_RESIZE));
        assert!(dispatcher.has_method(EDITOR_QUIT));
        assert!(dispatcher.has_method(EDITOR_SET_ACTIVE_BUFFER));

        // Server handlers
        assert!(dispatcher.has_method(SERVER_KILL));

        // state/layout handler added (#444)
        assert!(dispatcher.has_method(STATE_LAYOUT));

        // 17 implemented + 9 stubs + 13 debug (2 info + 4 inspect + 2 metrics + 4 log + 1 snapshot) = 40 total
        assert_eq!(dispatcher.method_count(), 40);
    }
}
