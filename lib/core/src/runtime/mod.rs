//! Runtime module - the main editor event loop and state management

mod action_handlers;
mod animation;
mod buffers;
mod context;
mod core;
mod enlist;
mod event_loop;
mod handlers;
mod mouse;
mod options;
mod profiles;
mod rendering;
mod rpc_handler;
mod sender;
mod snapshots;
mod subscribers;

pub use {
    context::{RuntimeContext, RuntimeContextExt},
    core::{FocusInputHandler, Runtime},
    enlist::{handle_command_line_input, handle_editor_input},
    sender::PrioritizedEventSender,
};
