//! Reovim Runner - Event Loop and Registry System
//!
//! This crate provides a clean architecture runner using the kernel-driver
//! type system from issue #213. It implements the "mechanism vs policy"
//! principle where:
//!
//! - **Mechanism** (this crate): Registries, event loop, fallback delegation
//! - **Policy** (modules): Mode implementations, commands, keybindings
//!
//! # Architecture
//!
//! ```text
//! runner/
//! ├── app.rs           - AppState (kernel + runtime state)
//! ├── fallback.rs      - InputFallbackHandler trait (mechanism)
//! ├── event_loop.rs    - Main event loop
//! └── registry/
//!     ├── mode.rs      - ModeRegistry
//!     ├── command.rs   - CommandRegistry
//!     └── keymap.rs    - KeymapRegistry
//! ```
//!
//! # Example
//!
//! ```ignore
//! use runner::{AppState, EventLoop, registry::*};
//! use runner::fallback::NoOpFallback;
//!
//! let app = AppState::new(kernel_context);
//! let mut event_loop = EventLoop::new(
//!     app,
//!     mode_registry,
//!     command_registry,
//!     keymap_registry,
//!     NoOpFallback,
//! );
//! event_loop.run()?;
//! ```

mod app;
mod event_loop;
mod fallback;
pub mod registry;

pub use {
    app::AppState,
    event_loop::{EventLoop, EventLoopError},
    fallback::{BeepFallback, FallbackResult, InputFallbackHandler, NoOpFallback},
};
