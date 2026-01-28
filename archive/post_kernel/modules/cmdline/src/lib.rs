//! Command line UI module (noice-style floating popup).
//!
//! This module provides a floating popup UI for command-line mode,
//! replacing the blind typing experience with a visible, interactive
//! command line interface.
//!
//! # Features
//!
//! - **Floating popup**: Displays at top of screen when entering `:`, `/`, `?`
//! - **Visible input**: Shows typed characters with cursor indicator
//! - **Basic editing**: Backspace, cursor movement via arrow keys
//! - **Rounded borders**: Modern UI with `╭╮╰╯` border characters
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │  CMDLINE MODULE (this crate)                                 POLICY     │
//! │  CmdlineModule, popup state, UI rendering, keybindings                  │
//! │  → Decides HOW the command line looks and behaves                       │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  OVERLAY LAYER (layout module)                              MECHANISM   │
//! │  OverlayZone, OverlayConstraints, Anchor                               │
//! │  → Provides overlay window infrastructure                               │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  SESSION DRIVER                                             MECHANISM   │
//! │  CmdlineState (active, prompt, cancelled), CmdlineBuffer (input)       │
//! │  → Provides state storage                                               │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Phase 1 Scope
//!
//! - Floating popup on `:`, `/`, `?` entry
//! - Basic editing: Backspace, Left, Right
//! - Execute on Enter, cancel on Escape
//! - Popup rendered via overlay layer
//!
//! # Future Phases
//!
//! - Phase 2: Command history, advanced editing, search preview
//! - Phase 3: Tab completion using `CommandQueryService`

pub mod commands;
pub mod ids;
pub mod popup;
pub mod ui;

use std::sync::Arc;

use {
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    tracing::info,
};

pub use {ids::MODULE, popup::SharedCmdlinePopupState};

/// The module identifier for the cmdline module.
pub const CMDLINE_MODULE: ModuleId = ModuleId::new("cmdline");

/// Cmdline UI module instance.
///
/// Provides a floating popup for command-line mode input.
///
/// # State Management
///
/// The module manages:
/// - `SharedCmdlinePopupState`: Tracks popup visibility and cached content
///
/// The actual input buffer (`CmdlineBuffer`) is in the runner layer,
/// accessed via `Session` methods.
pub struct CmdlineModule {
    /// Shared popup state for rendering.
    popup_state: Option<Arc<SharedCmdlinePopupState>>,
}

impl CmdlineModule {
    /// Create a new cmdline module.
    #[must_use]
    pub const fn new() -> Self {
        Self { popup_state: None }
    }
}

impl Default for CmdlineModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CmdlineModule {
    fn id(&self) -> ModuleId {
        CMDLINE_MODULE
    }

    fn name(&self) -> &'static str {
        "Cmdline UI Module"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 2)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        info!("Initializing cmdline UI module");

        // Create and register shared popup state
        let popup_state = Arc::new(SharedCmdlinePopupState::new());
        ctx.services.register(popup_state.clone());
        self.popup_state = Some(popup_state);

        info!("Cmdline UI module initialized - popup state registered");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        info!("Shutting down cmdline UI module");
        self.popup_state = None;
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CmdlineModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmdline_module_id() {
        assert_eq!(CMDLINE_MODULE.as_str(), "cmdline");
    }

    #[test]
    fn test_cmdline_module_identity() {
        let module = CmdlineModule::new();
        assert_eq!(module.id(), CMDLINE_MODULE);
        assert_eq!(module.name(), "Cmdline UI Module");
        assert_eq!(module.version(), Version::new(0, 9, 2));
    }

    #[test]
    fn test_cmdline_module_default() {
        let module = CmdlineModule::default();
        assert!(module.popup_state.is_none());
    }
}
