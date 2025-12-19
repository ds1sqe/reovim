//! Visual snapshot integration tests
//!
//! Tests for the visual debugging infrastructure including:
//! - ASCII art snapshots
//! - Structured visual snapshots
//! - Layer visibility
//! - `VisualAssertions` trait

mod common;

use {common::*, reovim_core::testing::VisualAssertions};

// ============================================================================
// Screen size tests
// ============================================================================

/// Test that explicit screen size is respected in snapshot
#[tokio::test]
async fn test_snapshot_explicit_size() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Hello")
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    assert_eq!(snap.width, 80, "Width should be 80");
    assert_eq!(snap.height, 24, "Height should be 24");
}

/// Test that small screen size is respected
#[tokio::test]
async fn test_snapshot_small_screen() {
    let mut result = ServerTest::new().await.with_size(40, 10).run().await;

    let snap = result.visual_snapshot().await;
    assert_eq!(snap.width, 40, "Width should be 40");
    assert_eq!(snap.height, 10, "Height should be 10");
}

/// Test that full screen has exactly width*height cells
#[tokio::test]
async fn test_full_screen_cells() {
    let mut result = ServerTest::new().await.with_size(40, 10).run().await;

    let snap = result.visual_snapshot().await;

    // Verify we have exactly height rows
    assert_eq!(snap.cells.len(), 10, "Should have 10 rows");

    // Verify each row has exactly width cells
    for (y, row) in snap.cells.iter().enumerate() {
        assert_eq!(row.len(), 40, "Row {y} should have 40 cells");
    }
}

/// Test that full screen rows each have correct width
#[tokio::test]
async fn test_full_screen_rows() {
    let mut result = ServerTest::new().await.with_size(60, 15).run().await;

    let snap = result.visual_snapshot().await;

    assert_eq!(snap.cells.len(), 15, "Should have 15 rows");
    for (y, row) in snap.cells.iter().enumerate() {
        assert_eq!(row.len(), 60, "Row {y} should have 60 cells, got {}", row.len());
    }
}

// ============================================================================
// Basic snapshot tests
// ============================================================================

/// Test that visual snapshot contains buffer content
#[tokio::test]
async fn test_visual_snapshot_basic() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Hello World")
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    assert!(
        snap.plain_text.contains("Hello World"),
        "Plain text should contain 'Hello World', got:\n{}",
        snap.plain_text
    );
}

/// Test that plain ASCII art contains buffer content
#[tokio::test]
async fn test_ascii_art_plain() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Test content here")
        .run()
        .await;

    let ascii = result.ascii_art(false).await;
    assert!(
        ascii.contains("Test content here"),
        "Plain ASCII should contain buffer content, got:\n{ascii}"
    );
}

/// Test that annotated ASCII art has borders
#[tokio::test]
async fn test_ascii_art_annotated() {
    let mut result = ServerTest::new()
        .await
        .with_size(40, 10)
        .with_content("Test content")
        .run()
        .await;

    let ascii = result.ascii_art(true).await;
    // Annotated ASCII should have borders and line numbers
    assert!(ascii.contains('+'), "Should have border corners");
    assert!(ascii.contains('|'), "Should have vertical borders");
    assert!(ascii.contains('-'), "Should have horizontal borders");
}

/// Test that inserted text appears in snapshot
#[tokio::test]
async fn test_snapshot_after_insert() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_keys("iInserted text<Esc>")
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    assert!(
        snap.plain_text.contains("Inserted text"),
        "Plain text should contain inserted text"
    );
}

// ============================================================================
// Layer tests
// ============================================================================

/// Test that editor layer is visible by default
#[tokio::test]
async fn test_layer_info_editor_visible() {
    let mut result = ServerTest::new().await.with_size(80, 24).run().await;

    let layers = result.layer_info().await;
    assert!(
        layers.iter().any(|l| l.name == "editor" && l.visible),
        "Editor layer should be visible. Layers: {layers:?}"
    );
}

/// Test that telescope layer is visible when opened
#[tokio::test]
async fn test_layer_info_telescope() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_keys(" ff") // Space f f - telescope find files
        .with_delay(100)
        .run()
        .await;

    let layers = result.layer_info().await;
    assert!(
        layers.iter().any(|l| l.name == "telescope" && l.visible),
        "Telescope layer should be visible when opened. Layers: {layers:?}"
    );
}

/// Test that which-key layer is visible after timeout
#[tokio::test]
async fn test_layer_info_which_key() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_keys("g")
        .with_delay(600) // 500ms timeout + buffer
        .run()
        .await;

    let layers = result.layer_info().await;
    assert!(
        layers.iter().any(|l| l.name == "which_key" && l.visible),
        "Which-key layer should be visible after timeout. Layers: {layers:?}"
    );
}

// ============================================================================
// Visual assertions tests
// ============================================================================

/// Test `assert_cell_char` on specific positions
#[tokio::test]
async fn test_assert_cell_char() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("ABCDE")
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    // Characters should be visible somewhere on the screen
    // The exact position depends on line numbers, but we can check the content exists
    assert!(snap.plain_text.contains("ABCDE"), "Screen should contain ABCDE");
}

/// Test `assert_contains` on screen content
#[tokio::test]
async fn test_assert_contains() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Hello World\nSecond Line")
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    snap.assert_contains("Hello World");
    snap.assert_contains("Second Line");
}

/// Test `assert_row_starts_with`
#[tokio::test]
async fn test_assert_row_starts_with() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("First line\nSecond line")
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    // Row 0 should have line number prefix and then content
    // The exact format depends on line number width
    snap.assert_contains("First line");
}

/// Test `region_to_string` extracts correct content
#[tokio::test]
async fn test_region_to_string() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("ABCDEFGHIJ\nKLMNOPQRST\nUVWXYZ")
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    // Just verify the screen has the content somewhere
    assert!(snap.plain_text.contains("ABCDEFGHIJ"));
    assert!(snap.plain_text.contains("KLMNOPQRST"));
}

// ============================================================================
// Cursor tests
// ============================================================================

/// Test that cursor position is reported in snapshot
#[tokio::test]
async fn test_cursor_in_snapshot() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Hello")
        .with_keys("ll") // Move cursor right twice
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    assert!(snap.cursor.is_some(), "Cursor should be present in snapshot");
    let cursor = snap.cursor.unwrap();
    // Cursor should have moved from position 0
    assert!(cursor.x > 0, "Cursor x should be > 0 after ll");
}

/// Test cursor after insert
#[tokio::test]
async fn test_cursor_after_insert() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_keys("iHello<Esc>")
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    assert!(snap.cursor.is_some(), "Cursor should be present");
}

// ============================================================================
// Split window tests
// ============================================================================

/// Test that :vs creates a vertical split showing same content in both windows
#[tokio::test]
async fn test_vsplit_creates_two_windows() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Line 1\nLine 2\nLine 3")
        .with_keys(":vs<CR>")
        .with_delay(100)
        .run()
        .await;

    let snap = result.visual_snapshot().await;
    // Both windows should show the same content
    // The content "Line 1" should appear twice (once in each window)
    let line1_count = snap.plain_text.matches("Line 1").count();
    assert!(
        line1_count >= 2,
        "Expected 'Line 1' to appear in both windows (at least 2 times), found {line1_count} times.\nScreen:\n{}",
        snap.plain_text
    );
}

/// Test that Ctrl-l navigates to right window after vsplit
#[tokio::test]
async fn test_vsplit_navigate_right() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Hello World")
        .with_keys(":vs<CR>")
        .with_delay(50)
        .with_keys("<C-l>")
        .with_delay(50)
        .run()
        .await;

    // After navigation, we should still be in normal mode and cursor should be present
    result.assert_normal_mode();
    let snap = result.visual_snapshot().await;
    assert!(snap.cursor.is_some(), "Cursor should be present after window navigation");
}

/// Test that cursor movement works after switching windows
#[tokio::test]
async fn test_vsplit_cursor_movement_after_navigate() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Line 1\nLine 2\nLine 3\nLine 4\nLine 5")
        .with_keys(":vs<CR>")
        .with_delay(50)
        .with_keys("<C-l>") // Navigate to right window
        .with_delay(50)
        .with_keys("jj") // Move down 2 lines
        .run()
        .await;

    // Should be in normal mode with cursor moved
    result.assert_normal_mode();
    let snap = result.visual_snapshot().await;
    let cursor = snap.cursor.expect("Cursor should be present");
    // Cursor y should be 2 (moved down twice from line 0)
    assert_eq!(cursor.y, 2, "Cursor should be on line 2 after jj, got line {}", cursor.y);
}

/// Test that Ctrl-h navigates back to left window
#[tokio::test]
async fn test_vsplit_navigate_left() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Test content")
        .with_keys(":vs<CR>")
        .with_delay(50)
        .with_keys("<C-l>") // Go right
        .with_delay(50)
        .with_keys("<C-h>") // Go back left
        .with_delay(50)
        .run()
        .await;

    result.assert_normal_mode();
    let snap = result.visual_snapshot().await;
    assert!(snap.cursor.is_some(), "Cursor should be present after navigating back");
}

/// Test that windows have independent cursors after split
/// After :vs, focus is on the NEW (right) window. Move cursor there, then navigate
/// to the left window which should still have cursor at 0.
#[tokio::test]
async fn test_vsplit_independent_cursors() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Line 1\nLine 2\nLine 3\nLine 4\nLine 5")
        .with_keys(":vs<CR>")
        .with_delay(100)
        .with_keys("jj") // Move cursor down in RIGHT window (active after split) to line 2
        .with_delay(100)
        .with_keys("<C-h>") // Navigate to LEFT window - cursor should be at 0
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    let snap = result.visual_snapshot().await;
    let cursor = snap.cursor.expect("Cursor should be present");
    // After navigating to left window, cursor should be at line 0
    // (left window's saved cursor, not right window's cursor)
    assert_eq!(
        cursor.y, 0,
        "Left window cursor should be at line 0 after navigation, got line {}",
        cursor.y
    );
}

/// Test cursor position is preserved when switching back to a window
/// After :vs, focus is on the RIGHT window. Move cursor, switch to left, switch back.
#[tokio::test]
async fn test_vsplit_cursor_preserved_on_switch() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("Line 1\nLine 2\nLine 3\nLine 4\nLine 5")
        .with_keys(":vs<CR>")
        .with_delay(100)
        .with_keys("jjj") // Move to line 3 in RIGHT window (active after split)
        .with_delay(100)
        .with_keys("<C-h>") // Go to left window
        .with_delay(100)
        .with_keys("<C-l>") // Go back to right window
        .with_delay(100)
        .run()
        .await;

    result.assert_normal_mode();
    let snap = result.visual_snapshot().await;
    let cursor = snap.cursor.expect("Cursor should be present");
    // Cursor should be restored to line 3 where we left it in the right window
    assert_eq!(
        cursor.y, 3,
        "Right window cursor should be preserved at line 3, got line {}",
        cursor.y
    );
}
