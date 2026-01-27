//! Which-key module for reovim.
//!
//! This module displays available keybindings in a popup overlay after a
//! configurable timeout when a prefix key is pressed. This helps users
//! discover and remember keybindings.
//!
//! # Architecture
//!
//! This is a **POLICY** module - it defines WHEN to show keybinding hints.
//! The kernel and drivers provide the mechanisms (keymap registry, overlays).
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  WHICH-KEY MODULE (this module)               POLICY        │
//! │  → When to show popup (timeout, ? suffix)                   │
//! │  → What to display (filtered bindings, formatting)          │
//! │  → How to position (bottom of screen)                       │
//! ├─────────────────────────────────────────────────────────────┤
//! │  MECHANISM LAYERS                             CAPABILITIES  │
//! │  KeymapRegistry (runner) - query available bindings         │
//! │  OverlayLayer (layout)   - display popup window             │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Features
//!
//! - **Timeout popup**: After pressing a prefix key (like `g`), shows available
//!   bindings after a configurable timeout (default 500ms)
//! - **Immediate popup**: Press `?` after a prefix (like `g?`) to show bindings
//!   immediately
//! - **Filtering**: Type to narrow down displayed bindings
//! - **Lock-free**: Uses `ArcSwap` for render thread performance
//!
//! # Example
//!
//! ```ignore
//! // User presses 'g'
//! // After 500ms, popup shows:
//! //   ┌─────── g ───────┐
//! //   │ g  → Go to top  │
//! //   │ d  → Go to def  │
//! //   │ f  → Go to file │
//! //   └─────────────────┘
//! ```

use std::sync::Arc;

use arc_swap::ArcSwap;
use reovim_kernel::api::v1::*;

pub mod commands;
pub mod config;
pub mod filter;
pub mod ids;
pub mod render;
pub mod service;
pub mod state;

pub use commands::{WhichKeyCloseCommand, WhichKeyFilterCommand, WhichKeyShowCommand};
pub use config::WhichKeyConfig;
pub use filter::{filter_bindings, spawn_saturator, CommandDescriptionProvider};
pub use ids::{MODULE, WHICH_KEY_CLOSE, WHICH_KEY_FILTER, WHICH_KEY_SHOW};
pub use render::{bottom_overlay_constraints, calculate_dimensions, render_popup};
pub use service::WhichKeyService;
pub use state::{BindingEntry, FilterRequest, SaturatorHandle, WhichKeyCache, WhichKeySessionExt, WhichKeyState, WhichKeyVisibility};

/// Module ID for which-key.
pub const MODULE_ID: &str = "which-key";

/// Which-key popup module.
///
/// Displays available keybindings after a prefix key is pressed.
/// State is created during init and the saturator task is spawned
/// when all modules are loaded (via on_all_loaded).
pub struct WhichKeyModule {
    /// Module configuration.
    config: WhichKeyConfig,
    /// Shared state (created during init).
    state: Option<WhichKeyState>,
}

impl WhichKeyModule {
    /// Create a new which-key module with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: WhichKeyConfig::new(),
            state: None,
        }
    }

    /// Create a new which-key module with custom configuration.
    #[must_use]
    pub fn with_config(config: WhichKeyConfig) -> Self {
        Self {
            config,
            state: None,
        }
    }

    /// Get a reference to the shared cache for external access.
    ///
    /// Returns `None` if the module hasn't been initialized.
    #[must_use]
    pub fn cache(&self) -> Option<Arc<ArcSwap<WhichKeyCache>>> {
        self.state.as_ref().map(|s| s.cache_handle())
    }

    /// Get the timeout in milliseconds.
    #[must_use]
    pub const fn timeout_ms(&self) -> u64 {
        self.config.timeout_ms
    }

    /// Check if the module is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

impl Default for WhichKeyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for WhichKeyModule {
    fn id(&self) -> ModuleId {
        ModuleId::new(MODULE_ID)
    }

    fn name(&self) -> &'static str {
        "Which-Key"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        if !self.config.enabled {
            pr_info!("Which-key module disabled by configuration");
            return ProbeResult::Success;
        }

        // Create state (cache and handle storage)
        self.state = Some(WhichKeyState::new());

        pr_info!(
            "Which-key module initialized (timeout: {}ms)",
            self.config.timeout_ms
        );
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        // Cancel any running timer
        if let Some(ref mut state) = self.state {
            state.cancel_timer();

            // Abort saturator task if running
            if let Some(ref handle) = state.saturator {
                handle.task.abort();
                tracing::debug!("which-key: saturator task aborted");
            }
        }

        pr_info!("Which-key module exiting");
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        if !self.config.enabled {
            return Vec::new();
        }

        vec![
            // Close popup when visible - Escape key
            // Note: This is registered for normal mode. The actual behavior
            // of closing the popup is implemented in the command handler.
            KeybindingRegistration::new("<Escape>", ids::WHICH_KEY_CLOSE)
                .with_modes(&["normal"])
                .with_description("Close which-key popup")
                .with_category("which-key")
                .with_priority(50), // High priority to handle before other Escape handlers
        ]
    }
}

// Generate FFI entry points for dynamic loading
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(WhichKeyModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = WhichKeyModule::new();
        assert_eq!(module.id().as_str(), "which-key");
    }

    #[test]
    fn test_module_name() {
        let module = WhichKeyModule::new();
        assert_eq!(module.name(), "Which-Key");
    }

    #[test]
    fn test_module_version() {
        let module = WhichKeyModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_module_with_config() {
        let config = WhichKeyConfig::new().with_timeout(750);
        let module = WhichKeyModule::with_config(config);
        assert_eq!(module.config.timeout_ms, 750);
    }

    #[test]
    fn test_module_default() {
        let module = WhichKeyModule::default();
        assert_eq!(module.config.timeout_ms, 500);
    }

    #[test]
    fn test_module_is_enabled() {
        let module = WhichKeyModule::new();
        assert!(module.is_enabled());
    }

    #[test]
    fn test_module_timeout_ms() {
        let module = WhichKeyModule::new();
        assert_eq!(module.timeout_ms(), 500);

        let config = WhichKeyConfig::new().with_timeout(1000);
        let module = WhichKeyModule::with_config(config);
        assert_eq!(module.timeout_ms(), 1000);
    }

    #[test]
    fn test_module_cache_none_before_init() {
        let module = WhichKeyModule::new();
        assert!(module.cache().is_none());
    }

    #[test]
    fn test_keybindings_when_enabled() {
        let module = WhichKeyModule::new();
        let bindings = module.keybindings();

        // Should have Escape binding for close
        assert!(!bindings.is_empty());

        let escape_binding = bindings.iter().find(|b| b.keys == "<Escape>");
        assert!(escape_binding.is_some());

        let escape = escape_binding.unwrap();
        assert_eq!(escape.command_id, ids::WHICH_KEY_CLOSE);
        assert!(escape.modes.contains(&"normal"));
        assert_eq!(escape.category, Some("which-key"));
    }

    #[test]
    fn test_keybindings_when_disabled() {
        let mut config = WhichKeyConfig::new();
        config.enabled = false;
        let module = WhichKeyModule::with_config(config);

        let bindings = module.keybindings();
        assert!(bindings.is_empty());
    }
}
