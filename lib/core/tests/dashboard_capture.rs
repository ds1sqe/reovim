//! Dashboard (landing page) capture tests
//!
//! Tests that verify the frame buffer capture system correctly captures
//! the landing page content when starting without a file.

mod common;

use {common::*, reovim_core::testing::VisualAssertions};

// ============================================================================
// Dashboard content capture tests
// ============================================================================

/// Test that the dashboard logo (ASCII art) is captured when starting without content
#[tokio::test]
async fn test_capture_dashboard_logo() {
    let mut result = ServerTest::new().await.with_size(80, 24).run().await;

    let snap = result.visual_snapshot().await;

    // The landing page logo is ASCII art, check for characteristic pattern
    // The logo contains: |_|  \___|\___/ \_/  |_|_| |_| |_|
    let plain_text = &snap.plain_text;
    assert!(
        plain_text.contains("___"),
        "Dashboard should contain ASCII art logo pattern, got:\n{plain_text}"
    );
}

/// Test that the dashboard version is captured
#[tokio::test]
async fn test_capture_dashboard_version() {
    let mut result = ServerTest::new().await.with_size(80, 24).run().await;

    let snap = result.visual_snapshot().await;

    // Should contain version info
    let plain_text = &snap.plain_text;
    assert!(
        plain_text.contains("version"),
        "Dashboard should contain 'version', got:\n{plain_text}"
    );
}

/// Test that dashboard hints are captured
#[tokio::test]
async fn test_capture_dashboard_hints() {
    let mut result = ServerTest::new().await.with_size(80, 24).run().await;

    let snap = result.visual_snapshot().await;

    // Should contain help hints
    snap.assert_contains(":q");
    snap.assert_contains("insert mode");
}

/// Test that the full dashboard is properly captured in cell grid format
#[tokio::test]
async fn test_capture_dashboard_cell_grid() {
    let mut result = ServerTest::new().await.with_size(80, 24).run().await;

    let snap = result.visual_snapshot().await;

    // Verify dimensions
    assert_eq!(snap.width, 80, "Width should be 80");
    assert_eq!(snap.height, 24, "Height should be 24");

    // Verify cell grid is populated
    assert_eq!(snap.cells.len(), 24, "Should have 24 rows");
    for (y, row) in snap.cells.iter().enumerate() {
        assert_eq!(row.len(), 80, "Row {y} should have 80 cells");
    }

    // The logo ASCII art should be somewhere in the rows
    // Check for characteristic pattern from the logo: "___"
    let has_logo_pattern = snap.cells.iter().any(|row| {
        let row_text: String = row.iter().map(|c| c.char).collect();
        row_text.contains("___")
    });
    assert!(has_logo_pattern, "Cell grid should contain ASCII art logo pattern");

    // Also check for version text
    let has_version = snap.cells.iter().any(|row| {
        let row_text: String = row.iter().map(|c| c.char).collect();
        row_text.contains("version")
    });
    assert!(has_version, "Cell grid should contain 'version' text");
}

/// Test that dashboard is captured correctly at different sizes
#[tokio::test]
async fn test_capture_dashboard_small_screen() {
    let mut result = ServerTest::new().await.with_size(60, 20).run().await;

    let snap = result.visual_snapshot().await;

    // Even on smaller screen, should capture content
    assert_eq!(snap.width, 60, "Width should be 60");
    assert_eq!(snap.height, 20, "Height should be 20");

    // Should still contain version (landing page adapts to size)
    let plain_text = &snap.plain_text;
    assert!(
        plain_text.contains("version"),
        "Smaller screen should still capture version info, got:\n{plain_text}"
    );
}

/// Test that ASCII art output contains dashboard elements
#[tokio::test]
async fn test_capture_dashboard_ascii_art() {
    let mut result = ServerTest::new().await.with_size(80, 24).run().await;

    let ascii = result.ascii_art(false).await;

    // ASCII art should contain version info and help hints
    assert!(ascii.contains("version"), "ASCII art should contain 'version', got:\n{ascii}");
    assert!(ascii.contains(":q"), "ASCII art should contain ':q' hint, got:\n{ascii}");
}

/// Test that annotated ASCII art has borders around dashboard
#[tokio::test]
async fn test_capture_dashboard_annotated() {
    let mut result = ServerTest::new().await.with_size(40, 10).run().await;

    let ascii = result.ascii_art(true).await;

    // Annotated should have borders
    assert!(ascii.contains('+'), "Should have border corners");
    assert!(ascii.contains('|'), "Should have vertical borders");
    assert!(ascii.contains('-'), "Should have horizontal borders");
}

// ============================================================================
// Dashboard capture after navigation tests
// ============================================================================

/// Test that dashboard is replaced after inserting content
#[tokio::test]
async fn test_capture_after_insert_replaces_dashboard() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_keys("iHello World<Esc>")
        .run()
        .await;

    let snap = result.visual_snapshot().await;

    // After inserting text, the dashboard should be replaced
    snap.assert_contains("Hello World");

    // The dashboard logo should not be visible when buffer has content
    // (The landing page is only shown for empty buffers)
}

/// Test that opening command mode preserves dashboard capture
#[tokio::test]
async fn test_capture_dashboard_with_command_mode() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_keys(":") // Enter command mode
        .run()
        .await;

    let snap = result.visual_snapshot().await;

    // Dashboard should still be captured (logo pattern and version)
    let plain_text = &snap.plain_text;
    assert!(
        plain_text.contains("version"),
        "Dashboard should be visible in command mode, got:\n{plain_text}"
    );
}
