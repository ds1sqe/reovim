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
//! - [`diff`]: Diff formatting for undo node edit preview
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
pub mod diff;
pub mod ids;
pub mod mode;
pub mod render;

use reovim_kernel::api::v1::{
    KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
};

/// Module identifier for the undotree module.
pub const UNDOTREE_MODULE: ModuleId = ModuleId::new("undotree");

pub use {
    command::{
        UndotreeCloseCommand, UndotreeCommand, UndotreeDownCommand, UndotreeGotoCommand,
        UndotreePreviewCommand, UndotreeUpCommand,
    },
    diff::{
        DiffLine, DiffLineType, DiffOptions, edit_summary, format_edits_as_diff,
        format_edits_as_diff_with_options, should_summarize,
    },
    mode::UndotreeMode,
    render::{RenderLine, UndotreeRenderer},
};

/// Undotree visualization module.
///
/// Provides the `:undotree` command and panel navigation keybindings.
pub struct UndotreeModule;

impl UndotreeModule {
    /// Create a new undotree module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for UndotreeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for UndotreeModule {
    fn id(&self) -> ModuleId {
        UNDOTREE_MODULE
    }

    fn name(&self) -> &'static str {
        "Undotree Visualization"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        keybindings()
    }
}

/// Keybindings for the undotree mode.
///
/// These keybindings are only active when the undotree panel is open
/// and focused (in undotree mode).
#[must_use]
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vec![
        // Navigation
        KeybindingRegistration::new("j", ids::UNDOTREE_DOWN)
            .with_modes(&["undotree"])
            .with_category("navigation")
            .with_description("Move selection down in undotree"),
        KeybindingRegistration::new("k", ids::UNDOTREE_UP)
            .with_modes(&["undotree"])
            .with_category("navigation")
            .with_description("Move selection up in undotree"),
        // Activation
        KeybindingRegistration::new("<CR>", ids::UNDOTREE_GOTO)
            .with_modes(&["undotree"])
            .with_category("action")
            .with_description("Navigate to selected node"),
        KeybindingRegistration::new("<Enter>", ids::UNDOTREE_GOTO)
            .with_modes(&["undotree"])
            .with_category("action")
            .with_description("Navigate to selected node"),
        // Preview
        KeybindingRegistration::new("p", ids::UNDOTREE_PREVIEW)
            .with_modes(&["undotree"])
            .with_category("preview")
            .with_description("Preview diff of selected node"),
        // Close panel
        KeybindingRegistration::new("q", ids::UNDOTREE_CLOSE)
            .with_modes(&["undotree"])
            .with_category("panel")
            .with_description("Close undotree panel"),
        KeybindingRegistration::new("<Esc>", ids::UNDOTREE_CLOSE)
            .with_modes(&["undotree"])
            .with_category("panel")
            .with_description("Close undotree panel"),
    ]
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(UndotreeModule);
