//! RPC method handlers.
//!
//! This module contains handlers for each supported RPC method.
//! Handlers are functions that take `RpcContext` and params,
//! returning a boxed future with the result.

mod input;
mod server;
mod state;
pub mod stub;

pub use {
    input::input_keys,
    server::server_kill,
    state::{state_cursor, state_mode},
    stub::{
        editor_quit, editor_resize, state_ascii_art, state_layer_info, state_microscope,
        state_screen, state_selection, state_telescope, state_visual_snapshot, state_windows,
    },
};
