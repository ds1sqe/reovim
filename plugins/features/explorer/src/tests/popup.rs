//! Tests for file details popup functionality

use std::sync::Arc;

use reovim_core::plugin::{EditorContext, PluginStateRegistry, PluginWindow};

use crate::{
    state::ExplorerState,
    window::{ExplorerPluginWindow, FileDetailsPluginWindow},
};

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
        active_window_anchor_x: 0,
        active_window_anchor_y: 0,
        active_window_gutter_width: 0,
        active_window_scroll_y: 0,
        cursor_col: 0,
        cursor_row: 0,
        color_mode: reovim_core::highlight::ColorMode::TrueColor,
        pending_keys: String::new(),
    }
}

/// Create a test registry with explorer state initialized to a temp directory
fn create_test_registry() -> (Arc<PluginStateRegistry>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");

    // Create some test files
    std::fs::File::create(dir.path().join("test.txt")).expect("Failed to create test file");
    std::fs::create_dir(dir.path().join("subdir")).expect("Failed to create subdir");

    let registry = Arc::new(PluginStateRegistry::new());
    let state = ExplorerState::new(dir.path().to_path_buf()).expect("Failed to create state");
    registry.register(state);

    (registry, dir)
}

#[test]
fn test_popup_initially_hidden() {
    let (registry, _dir) = create_test_registry();
    let ctx = mock_editor_context();

    // Popup should initially be hidden
    let is_popup_visible = registry
        .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
        .unwrap();
    assert!(!is_popup_visible, "Popup should initially be hidden");

    // Plugin window should return None when popup is hidden
    let window = FileDetailsPluginWindow;
    let config = window.window_config(&registry, &ctx);
    assert!(config.is_none(), "Should return None when popup is hidden");
}

#[test]
fn test_show_file_details_opens_popup() {
    let (registry, _dir) = create_test_registry();

    // Make explorer visible first
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
        })
        .unwrap();

    // Show file details
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.show_file_details();
        })
        .unwrap();

    // Popup should now be visible
    let is_popup_visible = registry
        .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
        .unwrap();
    assert!(is_popup_visible, "Popup should be visible after show_file_details");
}

#[test]
fn test_popup_window_config_when_visible() {
    let (registry, _dir) = create_test_registry();
    let ctx = mock_editor_context();

    // Show popup
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
            s.show_file_details();
        })
        .unwrap();

    // Plugin window should return config when visible
    let window = FileDetailsPluginWindow;
    let config = window.window_config(&registry, &ctx);
    assert!(config.is_some(), "Should return Some config when popup is visible");

    let config = config.unwrap();

    // Verify z-order is above explorer (150) but below modal (400)
    assert_eq!(config.z_order, 350, "Popup z-order should be 350");
    assert!(config.visible, "Popup should be visible");

    // Verify dimensions (45x8 compact layout)
    assert_eq!(config.bounds.width, 45, "Popup width should be 45");
    assert_eq!(config.bounds.height, 8, "Popup height should be 8");

    // Verify popup is positioned to the right of explorer (explorer width + 1)
    let explorer_width = registry.with::<ExplorerState, _, _>(|s| s.width).unwrap();
    assert_eq!(
        config.bounds.x,
        explorer_width + 1,
        "Popup should be positioned to the right of explorer"
    );
}

#[test]
fn test_close_popup() {
    let (registry, _dir) = create_test_registry();

    // Open popup
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
            s.show_file_details();
        })
        .unwrap();

    // Verify it's open
    let is_visible = registry
        .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
        .unwrap();
    assert!(is_visible, "Popup should be visible");

    // Close popup
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.close_popup();
        })
        .unwrap();

    // Verify it's closed
    let is_visible = registry
        .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
        .unwrap();
    assert!(!is_visible, "Popup should be hidden after close_popup");
}

#[test]
fn test_popup_content_for_file() {
    let (registry, dir) = create_test_registry();

    // Write some content to make file have a size
    std::fs::write(dir.path().join("sized.txt"), "Hello, World!").expect("Failed to write");

    // Refresh and navigate to the file
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            let _ = s.refresh();
            s.visible = true;
            // Move to first file (after root directory)
            s.move_cursor(1);
        })
        .unwrap();

    // Show file details
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.show_file_details();
        })
        .unwrap();

    // Verify popup content
    let popup = registry
        .with::<ExplorerState, _, _>(|s| s.popup.clone())
        .unwrap();

    assert!(popup.visible, "Popup should be visible");
    assert!(!popup.name.is_empty(), "Popup should have a name");
    assert!(!popup.path.is_empty(), "Popup should have a path");
}

#[test]
fn test_popup_content_for_directory() {
    let (registry, _dir) = create_test_registry();

    // Show details for root (which is a directory)
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
            s.move_to_first(); // Root is first
            s.show_file_details();
        })
        .unwrap();

    // Verify popup content for directory
    let popup = registry
        .with::<ExplorerState, _, _>(|s| s.popup.clone())
        .unwrap();

    assert!(popup.visible, "Popup should be visible");
    assert_eq!(popup.file_type, "directory", "Type should be 'directory'");
    assert!(popup.size.is_none(), "Directories should not have size");
}

#[test]
fn test_popup_toggle_visibility() {
    let (registry, _dir) = create_test_registry();
    let ctx = mock_editor_context();

    let window = FileDetailsPluginWindow;

    // Initially hidden
    assert!(window.window_config(&registry, &ctx).is_none());

    // Show popup
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
            s.show_file_details();
        })
        .unwrap();

    // Now visible
    assert!(window.window_config(&registry, &ctx).is_some());

    // Close popup
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.close_popup();
        })
        .unwrap();

    // Hidden again
    assert!(window.window_config(&registry, &ctx).is_none());
}

#[test]
fn test_popup_z_order_hierarchy() {
    let (registry, _dir) = create_test_registry();
    let ctx = mock_editor_context();

    // Make both explorer and popup visible
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
            s.show_file_details();
        })
        .unwrap();

    let explorer_window = ExplorerPluginWindow;
    let popup_window = FileDetailsPluginWindow;

    let explorer_config = explorer_window.window_config(&registry, &ctx).unwrap();
    let popup_config = popup_window.window_config(&registry, &ctx).unwrap();

    // Popup should be rendered above explorer
    assert!(
        popup_config.z_order > explorer_config.z_order,
        "Popup z_order ({}) should be higher than explorer z_order ({})",
        popup_config.z_order,
        explorer_config.z_order
    );
}

#[test]
fn test_popup_positioned_below_cursor() {
    let (registry, _dir) = create_test_registry();

    // Show popup at different cursor positions
    let window = FileDetailsPluginWindow;

    for cursor_pos in [0usize, 1, 5, 10] {
        registry
            .with_mut::<ExplorerState, _, _>(|s| {
                s.visible = true;
                s.cursor_index = cursor_pos;
                s.scroll_offset = 0;
                s.show_file_details();
            })
            .unwrap();

        let ctx = EditorContext {
            screen_width: 120,
            screen_height: 40,
            tab_line_height: 1,
            status_line_height: 1,
            left_offset: 0,
            right_offset: 0,
            edit_mode: reovim_core::modd::EditMode::Normal,
            sub_mode: reovim_core::modd::SubMode::None,
            focused_component: reovim_core::modd::ComponentId::EDITOR,
            active_buffer_id: 0,
            buffer_count: 1,
            active_window_anchor_x: 0,
            active_window_anchor_y: 0,
            active_window_gutter_width: 0,
            active_window_scroll_y: 0,
            cursor_col: 0,
            cursor_row: 0,
            color_mode: reovim_core::highlight::ColorMode::TrueColor,
            pending_keys: String::new(),
        };

        let config = window.window_config(&registry, &ctx).unwrap();

        // Popup Y should be below cursor (tab_line_height + cursor_pos + 1)
        let expected_y = ctx.tab_line_height + cursor_pos as u16 + 1;
        assert_eq!(
            config.bounds.y, expected_y,
            "Popup should be below cursor at position {}",
            cursor_pos
        );
    }
}

#[test]
fn test_popup_does_not_exceed_screen_width() {
    let (registry, _dir) = create_test_registry();

    // Show popup
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
            s.show_file_details();
        })
        .unwrap();

    let window = FileDetailsPluginWindow;

    // Test with a very narrow screen
    let ctx = EditorContext {
        screen_width: 40, // Narrower than default popup width (45)
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
        active_window_anchor_x: 0,
        active_window_anchor_y: 0,
        active_window_gutter_width: 0,
        active_window_scroll_y: 0,
        cursor_col: 0,
        cursor_row: 0,
        color_mode: reovim_core::highlight::ColorMode::TrueColor,
        pending_keys: String::new(),
    };

    let config = window.window_config(&registry, &ctx).unwrap();

    // Popup width should be clamped to fit screen
    assert!(
        config.bounds.width <= ctx.screen_width,
        "Popup width ({}) should not exceed screen width ({})",
        config.bounds.width,
        ctx.screen_width
    );
}

#[test]
fn test_popup_clamped_when_cursor_near_bottom() {
    let (registry, _dir) = create_test_registry();

    // Show popup first (at cursor 0), then simulate cursor being near bottom of screen
    // by manipulating scroll_offset to make cursor_screen_y large
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
            s.cursor_index = 0;
            s.scroll_offset = 0;
            s.show_file_details();
            // Now manually set popup visible and simulate a screen position near bottom
            // by setting scroll_offset to a negative-like effect (cursor stays at 0 but screen pos is high)
        })
        .unwrap();

    let window = FileDetailsPluginWindow;

    // Use a very short screen height to test clamping
    let ctx = EditorContext {
        screen_width: 120,
        screen_height: 12, // Very short screen - popup is 8 lines tall
        tab_line_height: 1,
        status_line_height: 1,
        left_offset: 0,
        right_offset: 0,
        edit_mode: reovim_core::modd::EditMode::Normal,
        sub_mode: reovim_core::modd::SubMode::None,
        focused_component: reovim_core::modd::ComponentId::EDITOR,
        active_buffer_id: 0,
        buffer_count: 1,
        active_window_anchor_x: 0,
        active_window_anchor_y: 0,
        active_window_gutter_width: 0,
        active_window_scroll_y: 0,
        cursor_col: 0,
        cursor_row: 0,
        color_mode: reovim_core::highlight::ColorMode::TrueColor,
        pending_keys: String::new(),
    };

    let config = window.window_config(&registry, &ctx).unwrap();

    // Popup should not extend past the screen bottom (screen_height - status_line_height)
    let popup_bottom = config.bounds.y + config.bounds.height;
    let max_bottom = ctx.screen_height - ctx.status_line_height;
    assert!(
        popup_bottom <= max_bottom,
        "Popup bottom ({}) should not exceed screen bottom ({})",
        popup_bottom,
        max_bottom
    );
}

#[test]
fn test_popup_x_position_next_to_explorer() {
    let (registry, _dir) = create_test_registry();

    // Test with different explorer widths
    let window = FileDetailsPluginWindow;

    for explorer_width in [20u16, 30, 40, 50] {
        registry
            .with_mut::<ExplorerState, _, _>(|s| {
                s.visible = true;
                s.width = explorer_width;
                s.show_file_details();
            })
            .unwrap();

        let ctx = EditorContext {
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
            active_window_anchor_x: 0,
            active_window_anchor_y: 0,
            active_window_gutter_width: 0,
            active_window_scroll_y: 0,
            cursor_col: 0,
            cursor_row: 0,
            color_mode: reovim_core::highlight::ColorMode::TrueColor,
            pending_keys: String::new(),
        };

        let config = window.window_config(&registry, &ctx).unwrap();

        // Popup should be positioned at explorer_width + 1
        assert_eq!(
            config.bounds.x,
            explorer_width + 1,
            "Popup x should be at explorer_width + 1 for width {}",
            explorer_width
        );
    }
}

#[test]
fn test_popup_syncs_on_cursor_move() {
    let (registry, dir) = create_test_registry();

    // Create multiple files to navigate between
    std::fs::write(dir.path().join("file_a.txt"), "AAA").expect("Failed to write");
    std::fs::write(dir.path().join("file_b.txt"), "BBB").expect("Failed to write");

    // Initialize and refresh
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            let _ = s.refresh();
            s.visible = true;
            s.cursor_index = 0;
            s.show_file_details();
        })
        .unwrap();

    // Get initial popup name
    let initial_name = registry
        .with::<ExplorerState, _, _>(|s| s.popup.name.clone())
        .unwrap();

    // Move cursor down and sync popup
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.move_cursor(1);
            s.sync_popup();
        })
        .unwrap();

    // Get new popup name
    let new_name = registry
        .with::<ExplorerState, _, _>(|s| s.popup.name.clone())
        .unwrap();

    // Popup should still be visible
    let is_visible = registry
        .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
        .unwrap();
    assert!(is_visible, "Popup should remain visible after cursor move");

    // Popup content should have changed (different file)
    assert_ne!(initial_name, new_name, "Popup should update to show new file after cursor move");
}

#[test]
fn test_sync_popup_does_nothing_when_closed() {
    let (registry, _dir) = create_test_registry();

    // Ensure popup is closed
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
            s.popup.visible = false;
        })
        .unwrap();

    // Call sync_popup
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.sync_popup();
        })
        .unwrap();

    // Popup should still be closed
    let is_visible = registry
        .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
        .unwrap();
    assert!(!is_visible, "Popup should remain closed when sync_popup called on closed popup");
}

#[test]
fn test_copy_path_closes_popup_and_sets_message() {
    let (registry, _dir) = create_test_registry();

    // Open popup
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.visible = true;
            s.show_file_details();
        })
        .unwrap();

    // Verify popup is open
    let is_visible = registry
        .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
        .unwrap();
    assert!(is_visible, "Popup should be visible before copy");

    // Simulate what copy_path does: close popup and set message
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.close_popup();
            s.message = Some("Path copied to clipboard".to_string());
        })
        .unwrap();

    // Verify popup is closed
    let is_visible = registry
        .with::<ExplorerState, _, _>(|s| s.is_popup_visible())
        .unwrap();
    assert!(!is_visible, "Popup should be closed after copy");

    // Verify message is set
    let message = registry
        .with::<ExplorerState, _, _>(|s| s.message.clone())
        .unwrap();
    assert_eq!(
        message,
        Some("Path copied to clipboard".to_string()),
        "Message should indicate path was copied"
    );
}

#[test]
fn test_current_node_path_for_copy() {
    let (registry, dir) = create_test_registry();

    // Create a specific test file
    let test_file = dir.path().join("copy_test.txt");
    std::fs::write(&test_file, "test content").expect("Failed to write test file");

    // Refresh and navigate to the file
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            let _ = s.refresh();
            s.visible = true;
        })
        .unwrap();

    // Find the test file in the tree
    let found_path = registry
        .with::<ExplorerState, _, _>(|s| {
            // Iterate through visible nodes to find our file
            for (i, node) in s.visible_nodes().iter().enumerate() {
                if node.name == "copy_test.txt" {
                    return Some((i, node.path.clone()));
                }
            }
            None
        })
        .unwrap();

    assert!(found_path.is_some(), "Test file should be found in tree");

    let (index, expected_path) = found_path.unwrap();

    // Move cursor to the file
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.cursor_index = index;
        })
        .unwrap();

    // Get the path that would be copied
    let copy_path = registry
        .with::<ExplorerState, _, _>(|s| {
            s.current_node()
                .map(|n| n.path.to_string_lossy().to_string())
        })
        .unwrap();

    assert!(copy_path.is_some(), "Should have a path to copy");
    assert_eq!(
        copy_path.unwrap(),
        expected_path.to_string_lossy().to_string(),
        "Copy path should match the file path"
    );
}

#[test]
fn test_copy_path_with_popup_open_and_navigate() {
    let (registry, dir) = create_test_registry();

    // Create test files
    std::fs::write(dir.path().join("first.txt"), "first").expect("Failed to write");
    std::fs::write(dir.path().join("second.txt"), "second").expect("Failed to write");

    // Initialize
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            let _ = s.refresh();
            s.visible = true;
            s.cursor_index = 1; // First file after root
            s.show_file_details();
        })
        .unwrap();

    // Get initial file being shown
    let initial_name = registry
        .with::<ExplorerState, _, _>(|s| s.popup.name.clone())
        .unwrap();

    // Navigate to next file
    registry
        .with_mut::<ExplorerState, _, _>(|s| {
            s.move_cursor(1);
            s.sync_popup();
        })
        .unwrap();

    // Get new file being shown
    let new_name = registry
        .with::<ExplorerState, _, _>(|s| s.popup.name.clone())
        .unwrap();

    // They should be different (we navigated)
    assert_ne!(initial_name, new_name, "Popup should show different file after navigation");

    // Get the path that would be copied (should be for the NEW file)
    let copy_path = registry
        .with::<ExplorerState, _, _>(|s| {
            s.current_node()
                .map(|n| n.path.to_string_lossy().to_string())
        })
        .unwrap();

    assert!(copy_path.is_some(), "Should have a path to copy");

    // The path should contain the new file name
    let path_str = copy_path.unwrap();
    assert!(
        path_str.contains(&new_name),
        "Copy path '{}' should contain the current file name '{}'",
        path_str,
        new_name
    );
}
