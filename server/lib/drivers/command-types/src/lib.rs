//! Command types shared between command and session drivers.
//!
//! This crate provides the fundamental types for the command system:
//! - [`CommandContext`] - Carries all inputs for command execution
//! - [`CommandResult`] - Result of command execution
//! - [`MotionType`] - Motion classification for operator-pending mode
//! - [`ArgSpec`], [`ArgKind`], [`ArgValue`] - Argument specifications
//!
//! # Design
//!
//! These types are extracted to a separate crate to break the circular
//! dependency between `reovim-driver-command` and `reovim-driver-session`.
//! Both drivers can depend on this crate without creating a cycle.

mod args;
mod context;
mod motion;
mod result;
mod signal;

pub use {
    args::{ArgKind, ArgSpec, ArgValue},
    context::CommandContext,
    motion::MotionType,
    result::CommandResult,
    signal::RuntimeSignal,
};
