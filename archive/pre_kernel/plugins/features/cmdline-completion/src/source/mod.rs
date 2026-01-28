//! Completion sources for command-line

mod command;
mod path;

pub use {command::complete_commands, path::complete_paths};
