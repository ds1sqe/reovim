//! Completion sources
//!
//! This module contains built-in completion sources.
//! Sources implement the `SourceSupport` trait from `crate::registry`.

pub mod buffer;

pub use buffer::BufferWordsSource;
