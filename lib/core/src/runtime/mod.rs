//! Runtime module - the main editor event loop and state management

mod core;
mod enlist;
mod event_loop;
mod handlers;

pub use {
    core::{FocusInputHandler, Runtime},
    enlist::{handle_command_line_input, handle_editor_input, handle_telescope_input},
};
