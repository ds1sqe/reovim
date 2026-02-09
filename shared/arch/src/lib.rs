#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Platform abstraction layer for reovim.
//!
//! Linux equivalent: `arch/`
//!
//! This crate provides platform-agnostic traits for terminal I/O,
//! signal handling, and input events. All code above this layer
//! is platform-independent.
//!
//! # Architecture
//!
//! - `traits`: Platform-agnostic trait definitions and types
//! - `error`: Error types for arch operations
//! - `sync`: Synchronization primitives (`RwLock`, `Mutex`)
//! - `dirs`: Platform-specific directory paths
//! - `unix`: Unix/Linux implementation using crossterm (cfg(unix))
//! - `windows`: Windows implementation stubs (cfg(windows))
//!
//! # Usage
//!
//! The crate provides platform-specific type aliases for convenience:
//!
//! ```ignore
//! use reovim_arch::{PlatformTerminal, PlatformInputSource, Terminal, InputSource};
//!
//! let mut term = PlatformTerminal::new();
//! let size = term.size()?;
//! println!("Terminal size: {}x{}", size.cols, size.rows);
//! ```
//!
//! # Platform Support
//!
//! - **Unix/Linux/macOS**: Full support via crossterm
//! - **Windows**: Stub implementation (todo!() for all methods)

pub mod dirs;
pub mod error;
pub mod palette;
pub mod sync;
pub mod traits;

// Re-export all public types from traits (including ParseColorError, ParseColorErrorKind)
pub use traits::*;

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;

// Platform-specific type aliases for convenience
#[cfg(unix)]
pub use unix::{
    UnixInputSource as PlatformInputSource, UnixLocalListener as PlatformLocalListener,
    UnixLocalStream as PlatformLocalStream, UnixSignalHandler as PlatformSignalHandler,
    UnixTerminal as PlatformTerminal, process_exists,
};

#[cfg(windows)]
pub use windows::{
    WindowsInputSource as PlatformInputSource, WindowsLocalListener as PlatformLocalListener,
    WindowsLocalStream as PlatformLocalStream, WindowsSignalHandler as PlatformSignalHandler,
    WindowsTerminal as PlatformTerminal, process_exists,
};
