//! Adapter module for common client model integration.
//!
//! This module provides adapters between the common client model types
//! (`reovim-client-model`) and TUI-specific types (`reovim-driver-display`).
//!
//! # Architecture
//!
//! The adapter module follows the "extension, not replacement" philosophy:
//! - Common model types are used for data interchange (wire format)
//! - TUI keeps its sophisticated compositor for rendering
//! - Adapters convert between wire format and TUI types
//!
//! # Usage
//!
//! The [`TuiAdapterFactory`] provides a convenient way to create all adapters:
//!
//! ```ignore
//! let compositor = Arc::new(Mutex::new(my_compositor));
//! let factory = TuiAdapterFactory::new(compositor, active_layer);
//!
//! // Create adapters
//! let layout = factory.create_layout();
//! let focus = factory.create_focus_manager(initial_viewport);
//! ```
//!
//! # Modules
//!
//! - [`layout`] - Layout trait implementation wrapping TUI compositor
//! - [`panel`] - Panel trait implementation wrapping TUI View
//! - [`focus`] - Focus manager for panel/overlay focus tracking

use std::sync::{Arc, Mutex};

use reovim_driver_display::layout::{LayerId, RootCompositor};

pub mod focus;
pub mod layout;
pub mod panel;

// Re-export key types for convenience
pub use {focus::TuiFocusManager, layout::TuiLayoutAdapter, panel::TuiPanel};

/// Factory for creating TUI adapters from a compositor.
///
/// Provides a convenient way to create multiple adapters that share
/// the same underlying compositor.
///
/// # Type Parameter
///
/// * `C` - The compositor type (must implement `RootCompositor + 'static`)
pub struct TuiAdapterFactory<C: RootCompositor + 'static> {
    /// The underlying root compositor.
    compositor: Arc<Mutex<C>>,
    /// The active layer for tiled operations.
    active_layer: LayerId,
}

impl<C: RootCompositor + 'static> TuiAdapterFactory<C> {
    /// Create a new adapter factory.
    ///
    /// # Arguments
    ///
    /// * `compositor` - The root compositor to wrap
    /// * `active_layer` - The layer ID for tiled window operations
    #[must_use]
    pub const fn new(compositor: Arc<Mutex<C>>, active_layer: LayerId) -> Self {
        Self {
            compositor,
            active_layer,
        }
    }

    /// Get a reference to the compositor.
    #[must_use]
    pub fn compositor(&self) -> Arc<Mutex<C>> {
        Arc::clone(&self.compositor)
    }

    /// Get the active layer ID.
    #[must_use]
    pub const fn active_layer(&self) -> LayerId {
        self.active_layer
    }

    /// Create a layout adapter.
    #[must_use]
    pub fn create_layout(&self) -> TuiLayoutAdapter<C> {
        TuiLayoutAdapter::new(Arc::clone(&self.compositor), self.active_layer)
    }

    /// Create a focus manager.
    ///
    /// # Arguments
    ///
    /// * `initial_viewport` - The initially focused viewport ID
    #[must_use]
    pub fn create_focus_manager(&self, initial_viewport: u64) -> TuiFocusManager<C> {
        TuiFocusManager::new(Arc::clone(&self.compositor), initial_viewport)
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
