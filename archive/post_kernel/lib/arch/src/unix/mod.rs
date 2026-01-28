//! Unix/Linux platform implementation.
//!
//! Linux equivalent: `arch/x86/`, `arch/arm64/`, etc.
//!
//! This module provides Unix-specific implementations of the platform traits
//! using crossterm as the underlying terminal library.

mod convert;
mod input;
pub mod local;
mod signal;
mod terminal;

pub use {
    convert::{
        convert_color_from_crossterm, convert_color_to_crossterm, convert_event, convert_key_code,
        convert_key_event, convert_modifiers,
    },
    input::UnixInputSource,
    local::{UnixLocalListener, UnixLocalStream, process_exists},
    signal::UnixSignalHandler,
    terminal::UnixTerminal,
};
