//! Tests for explorer toggle functionality

use std::sync::Arc;

use reovim_core::plugin::{EditorContext, PluginStateRegistry, PluginWindow};

use crate::{state::ExplorerState, window::ExplorerPluginWindow};

/// Create a mock EditorContext for testing
fn mock_editor_context() -> EditorContext {
    EditorContext {
        screen_width: 120,
        screen_height: 40,
        tab_line_height: 0,
        status_line_height: 1,
        left_offset: 0,
        right_offset: 0,
        edit_mode: reovim_core::modd::EditMode::Normal,
        sub_mode: reovim_core::modd::SubMode::None,
        focused_component: reovim_core::modd::ComponentId::EDITOR,
        active_buffer_id: 0,
        buffer_count: 1,
        color_mode: reovim_core::highlight::ColorMode::TrueColor,
        pending_keys: String::new(),
    }
}

#[test]
fn test_explorer_toggle() {
    // Create state registry and register explorer state
    let registry = Arc::new(PluginStateRegistry::new());
    let ctx = mock_editor_context();

    if let Ok(cwd) = std::env::current_dir()
        && let Ok(state) = ExplorerState::new(cwd)
    {
        registry.register(state);
    } else {
        panic!("Failed to create ExplorerState");
    }

    // Initially, explorer should not be visible
    let is_visible = registry
        .with::<ExplorerState, _, _>(|explorer| explorer.visible)
        .unwrap();
    assert!(!is_visible, "Explorer should initially be hidden");

    // Plugin window should return None when hidden
    let window = ExplorerPluginWindow;
    let config = window.window_config(&registry, &ctx);
    assert!(config.is_none(), "Should return None when hidden");

    // Toggle visibility to show
    registry
        .with_mut::<ExplorerState, _, _>(|explorer| {
            explorer.toggle_visibility();
        })
        .unwrap();

    // Explorer should now be visible
    let is_visible = registry
        .with::<ExplorerState, _, _>(|explorer| explorer.visible)
        .unwrap();
    assert!(is_visible, "Explorer should be visible after toggle");

    // Plugin window should return Some config when visible
    let config = window.window_config(&registry, &ctx);
    assert!(config.is_some(), "Should return Some config when visible");

    // Verify window properties
    let config = config.unwrap();
    assert_eq!(config.z_order, 150, "Explorer z-order should be 150");
    assert!(config.visible, "Explorer should be visible");

    // Toggle again to hide
    registry
        .with_mut::<ExplorerState, _, _>(|explorer| {
            explorer.toggle_visibility();
        })
        .unwrap();

    // Explorer should be hidden again
    let is_visible = registry
        .with::<ExplorerState, _, _>(|explorer| explorer.visible)
        .unwrap();
    assert!(!is_visible, "Explorer should be hidden after second toggle");

    // Plugin window should return None again
    let config = window.window_config(&registry, &ctx);
    assert!(config.is_none(), "Should return None when hidden again");
}

#[test]
fn test_explorer_buffer_provider_respects_visibility() {
    use {
        crate::provider::ExplorerBufferProvider,
        reovim_core::content::{BufferContext, PluginBufferProvider},
    };

    let registry = Arc::new(PluginStateRegistry::new());

    if let Ok(cwd) = std::env::current_dir()
        && let Ok(state) = ExplorerState::new(cwd)
    {
        registry.register(state);
    } else {
        panic!("Failed to create ExplorerState");
    }

    let provider = ExplorerBufferProvider;
    let ctx = BufferContext {
        buffer_id: 0,
        width: 30,
        height: 20,
        state: &registry,
    };

    // When hidden, should return no lines
    let lines = provider.get_lines(&ctx);
    assert_eq!(lines.len(), 0, "Should have no lines when hidden");

    // Toggle to visible
    registry
        .with_mut::<ExplorerState, _, _>(|explorer| {
            explorer.toggle_visibility();
        })
        .unwrap();

    // When visible, should return lines (padded to height)
    let lines = provider.get_lines(&ctx);
    assert_eq!(lines.len(), 20, "Should return lines matching height when visible");
}
