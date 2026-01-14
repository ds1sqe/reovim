//! RPC method handlers.
//!
//! This module contains handlers for each supported RPC method.
//! Handlers are functions that take `RpcContext` and params,
//! returning a boxed future with the result.

mod buffer;
mod command;
mod input;
mod module;
mod screen;
mod server;
mod state;
pub mod stub;

pub use {
    buffer::{buffer_get_content, buffer_list, buffer_open_file, buffer_set_content},
    command::command_execute,
    input::input_keys,
    module::{module_list, module_load, module_reload, module_unload},
    screen::state_screen_content,
    server::server_kill,
    state::{state_cursor, state_mode},
    stub::{
        editor_quit, editor_resize, state_ascii_art, state_layer_info, state_microscope,
        state_screen, state_selection, state_telescope, state_visual_snapshot, state_windows,
    },
};
