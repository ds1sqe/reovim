//! Terminal I/O abstraction for testing
//!
//! This module provides traits and implementations for abstracting terminal I/O,
//! enabling integration tests without requiring a real terminal.

pub mod input;
pub mod output;

pub use input::{ChannelKeySource, EventStreamKeySource, KeySource, MockKeySource};
pub use output::MockOutput;
