//! Platform abstraction layer for reovim.
//!
//! Linux equivalent: `arch/`
//!
//! This crate provides platform-agnostic traits for terminal I/O,
//! signal handling, and memory operations. All code above this layer
//! is platform-independent.
//!
//! # Architecture
//!
//! - `traits`: Platform-agnostic trait definitions
//! - `error`: Error types for arch operations
//! - `unix`: Unix/Linux implementation (cfg(unix))
//! - `windows`: Windows implementation (cfg(windows))

pub mod error;
pub mod traits;

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;
