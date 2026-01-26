//! RPC method handlers.
//!
//! This module contains handlers for each supported RPC method.
//! Handlers are functions that take `RpcContext` and params,
//! returning a boxed future with the result.

mod buffer;
mod command;
mod editor;
mod input;
mod module;
mod screen;
mod server;
mod state;
pub mod stub;
mod tui;

#[cfg(test)]
pub mod test_utils;

pub use {
    buffer::{
        buffer_get_content, buffer_list, buffer_open_file, buffer_set_content, buffer_write_file,
    },
    command::command_execute,
    editor::{editor_quit, editor_resize, editor_set_active_buffer},
    input::input_keys,
    module::{module_list, module_load, module_reload, module_unload},
    screen::state_screen_content,
    server::server_kill,
    state::{state_cursor, state_layout, state_mode, state_screen, state_selection},
    stub::{
        state_ascii_art, state_layer_info, state_microscope, state_telescope,
        state_visual_snapshot, state_windows,
    },
    tui::tui_capture,
};
