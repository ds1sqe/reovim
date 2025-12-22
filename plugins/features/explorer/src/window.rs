//! Explorer window provider

use std::sync::Arc;

use reovim_core::{
    content::WindowContentSource,
    plugin::{PluginStateRegistry, WindowProvider},
    screen::{
        Position,
        window::{Anchor, Window},
    },
};

use crate::{provider::ExplorerBufferProvider, state::ExplorerState};

/// Window provider for the explorer
pub struct ExplorerWindowProvider;

impl WindowProvider for ExplorerWindowProvider {
    fn get_windows(&self, state: &Arc<PluginStateRegistry>) -> Vec<Window> {
        // Check if explorer is visible
        let is_visible = state
            .with::<ExplorerState, _, _>(|explorer| explorer.visible)
            .unwrap_or(false);

        tracing::debug!("ExplorerWindowProvider::get_windows() called, visible={}", is_visible);

        if !is_visible {
            tracing::debug!("ExplorerWindowProvider: returning 0 windows (hidden)");
            return Vec::new();
        }

        // Get explorer width from state
        let width = state
            .with::<ExplorerState, _, _>(|explorer| explorer.width)
            .unwrap_or(30);

        tracing::debug!("ExplorerWindowProvider: creating explorer window with width={}", width);

        // Create explorer window
        let window = Window {
            id: usize::MAX, // Temporary ID for explorer
            source: WindowContentSource::PluginBuffer {
                buffer_id: 0, // Explorer doesn't use real buffer
                buffer_anchor: Anchor { x: 0, y: 0 },
                provider: Arc::new(ExplorerBufferProvider),
            },
            anchor: Anchor { x: 0, y: 1 }, // Below tab line
            width,
            height: 50,   // Will be adjusted by layout (TODO: Get from screen height)
            z_order: 150, // Between editor windows (100-199) and overlays (200+)
            is_active: false,
            is_floating: true,
            line_number: None,
            scrollbar_enabled: false,
            cursor: Position { x: 0, y: 0 },
            desired_col: None,
            border_config: None,
        };

        vec![window]
    }
}
