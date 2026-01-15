//! Undotree Visualization Module for Reovim
//!
//! This module provides Vim-style `:undotree` visualization, allowing users
//! to see and navigate the branching undo history in a side panel.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - **Mechanism** (kernel): `UndoTree` data structure, traversal APIs
//! - **Policy** (this module): Visualization, navigation commands
//!
//! # Components
//!
//! - [`command`]: Commands for toggling and navigating the undotree panel
//! - [`render`]: ASCII tree rendering with branch visualization
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_undotree::command::UndotreeCommand;
//! use reovim_module_undotree::render::UndotreeRenderer;
//!
//! // Register the :undotree command
//! registry.register(UndotreeCommand);
//!
//! // Render a tree
//! let renderer = UndotreeRenderer::new(30);
//! let lines = renderer.render(&tree, tree.current_index());
//! ```

pub mod command;
pub mod mode;
pub mod render;

use reovim_kernel::api::v1::ModuleId;

/// Module identifier for the undotree module.
pub const UNDOTREE_MODULE: ModuleId = ModuleId::new("undotree");

pub use {
    command::{
        UndotreeCloseCommand, UndotreeCommand, UndotreeDownCommand, UndotreeGotoCommand,
        UndotreeUpCommand,
    },
    mode::UndotreeMode,
    render::{RenderLine, UndotreeRenderer},
};
