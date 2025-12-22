//! Runtime module - the main editor event loop and state management

mod context;
mod core;
mod enlist;
mod event_loop;
mod handlers;

pub use {
    context::{RuntimeContext, RuntimeContextExt},
    core::{FocusInputHandler, Runtime},
    enlist::{handle_command_line_input, handle_editor_input},
};
