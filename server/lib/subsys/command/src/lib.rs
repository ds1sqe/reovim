#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Command subsystem contracts for reovim — command framework.
//!
//! Core traits and types for the command system. The execution trait
//! `CommandHandler` lives in `reovim_driver_command` because it
//! requires domain-specific types.

pub mod capabilities;
mod name_index;
mod parse;
mod query;
mod traits;

pub use {
    name_index::{AmbiguousPrefix, CommandNameIndex},
    parse::{ArgError, ParsedCmdline, bind_args, parse_cmdline, tokenize_args},
    query::{CommandInfo, CommandQueryProvider, CommandQueryService},
    traits::{Command, CommandPriority},
};

// Re-export from command-types for convenience
pub use reovim_subsys_command_types::{
    ArgKind, ArgSpec, ArgValue, CommandContext, CommandResult, MotionType, RuntimeSignal,
};
