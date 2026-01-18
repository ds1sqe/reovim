//! Core Editor Module for Reovim
//!
//! This module provides basic editor commands (cursor movement, text operations)
//! that form the foundation of vim-style editing.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - **Mechanism** (kernel/drivers): Mode/Command traits, registries
//! - **Policy** (this module): Command implementations
//!
//! # Components
//!
//! - [`command`]: Cursor movement, delete, yank, paste, and other text operations
//! - [`ResolverRegistry`]: Motion and text object resolvers
//!
//! # Note
//!
//! Mode definitions and mode-specific commands (enter insert, exit to normal,
//! etc.) have been moved to `reovim_module_vim`. This module provides
//! policy-agnostic editor commands; mode identities and transitions are in vim.
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_editor::command::{CursorDown, DeleteLine};
//!
//! // Register editor commands in your application
//! for cmd in reovim_module_editor::command::all_commands() {
//!     command_registry.register(cmd);
//! }
//! ```

pub mod command;
pub mod display_lines;
pub mod resolver;

pub use resolver::ResolverRegistry;
