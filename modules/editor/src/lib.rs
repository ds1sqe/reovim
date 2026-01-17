//! Core Editor Module for Reovim
//!
//! This module provides the fundamental editor modes (Normal, Insert) and
//! basic commands (cursor movement, mode switching) that form the foundation
//! of vim-style editing.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - **Mechanism** (kernel/drivers): Mode/Command traits, registries
//! - **Policy** (this module): Actual mode behavior, command implementations
//!
//! # Components
//!
//! - [`EditorMode`]: Normal and Insert mode implementations
//! - [`command`]: Cursor movement and mode switching commands
//! - [`EditorFallbackHandler`]: Character insertion in Insert mode
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_editor::{EditorMode, EditorFallbackHandler};
//! use reovim_module_editor::command::{CursorDown, EnterInsertMode};
//!
//! // Register modes
//! let normal = EditorMode::Normal;
//! let insert = EditorMode::Insert;
//!
//! // Use fallback handler for char insertion
//! let fallback = EditorFallbackHandler;
//! ```

pub mod command;
pub mod display_lines;
mod fallback;
mod mode;
pub mod resolver;
pub mod visual;

pub use {
    fallback::EditorFallbackHandler,
    mode::{EDITOR_MODULE, EditorMode},
    resolver::{
        ResolverRegistry, VimInsertResolver, VimNormalResolver, VimOperatorPendingResolver,
    },
};
