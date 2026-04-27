//! Focus management trait implementation for TUI.
//!
//! This module provides an adapter between the common client model's `FocusManager`
//! trait and TUI's `RootCompositor` focus handling.

use std::sync::{Arc, Mutex};

use {
    reovim_client_model::traits::{Focus, FocusManager},
    reovim_driver_display::{WindowId, layout::RootCompositor},
};

/// Adapter implementing the common `FocusManager` trait for TUI.
///
/// Tracks focus state and synchronizes with the underlying `RootCompositor`.
/// Supports focus transitions between panels (editor viewports) and overlays
/// (popups, menus).
///
/// # ID Conversion
///
/// The common model uses `u64` for viewport IDs while TUI uses `WindowId` (usize).
/// This adapter handles the conversion transparently.
pub struct TuiFocusManager<C: RootCompositor + 'static> {
    /// The underlying root compositor (for syncing panel focus).
    compositor: Arc<Mutex<C>>,
    /// Current focus state.
    current: Focus,
    /// Last focused panel (for `return_to_panel`).
    last_panel: u64,
}

impl<C: RootCompositor + 'static> TuiFocusManager<C> {
    /// Create a new focus manager.
    ///
    /// # Arguments
    ///
    /// * `compositor` - The root compositor to sync panel focus with
    /// * `initial_viewport` - The initially focused viewport ID
    #[must_use]
    pub const fn new(compositor: Arc<Mutex<C>>, initial_viewport: u64) -> Self {
        Self {
            compositor,
            current: Focus::Panel(initial_viewport),
            last_panel: initial_viewport,
        }
    }

    /// Get a reference to the underlying compositor.
    #[must_use]
    pub fn compositor(&self) -> Arc<Mutex<C>> {
        Arc::clone(&self.compositor)
    }

    /// Convert a `u64` to `WindowId`.
    #[must_use]
    const fn u64_to_window_id(id: u64) -> WindowId {
        #[allow(clippy::cast_possible_truncation)]
        WindowId::from_raw(id as usize)
    }

    /// Sync the focus state with the compositor.
    ///
    /// Only syncs if the current focus is on a panel (compositor doesn't
    /// know about overlay focus).
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn sync_with_compositor(&self) {
        if let Focus::Panel(viewport_id) = &self.current {
            let window_id = Self::u64_to_window_id(*viewport_id);
            if let Ok(mut compositor) = self.compositor.lock() {
                compositor.set_focus(window_id);
            }
        }
    }
}

#[allow(clippy::significant_drop_tightening)]
impl<C: RootCompositor + 'static> FocusManager for TuiFocusManager<C> {
    fn current(&self) -> &Focus {
        &self.current
    }

    fn focus_panel(&mut self, viewport_id: u64) {
        self.last_panel = viewport_id;
        self.current = Focus::Panel(viewport_id);
        self.sync_with_compositor();
    }

    fn focus_overlay(&mut self, overlay_id: &str) {
        // Remember the last panel before switching to overlay
        if let Focus::Panel(id) = self.current {
            self.last_panel = id;
        }
        self.current = Focus::Overlay(overlay_id.to_string());
        // Note: Compositor focus stays on the panel - overlays are
        // rendered on top but the panel retains compositor focus
    }

    fn return_to_panel(&mut self) {
        self.current = Focus::Panel(self.last_panel);
        self.sync_with_compositor();
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
#[path = "focus_tests.rs"]
mod tests;
