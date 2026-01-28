//! Undotree panel integration tests.
//!
//! Tests for the `:undotree` command and panel interaction.
//! These tests verify the complete workflow from opening the panel
//! to navigating and applying undo states.

use runner::testing::{IntegrationTest, TestResult};

/// Assert the result is in undotree mode.
fn assert_undotree_mode(result: &TestResult) {
    assert!(
        result.edit_mode.to_lowercase().contains("undotree")
            || result.mode_display.to_uppercase().contains("UNDOTREE"),
        "Expected undotree mode, got: {} ({})",
        result.mode_display,
        result.edit_mode
    );
}

// ============================================================================
// :undotree Command Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_command_opens_panel() {
    // :undotree should open the undotree panel and enter undotree mode
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys(":undotree<CR>")
        .run()
        .await;

    assert_undotree_mode(&result);
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_toggle() {
    // :undotree twice should open then close
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys(":undotree<CR>:undotree<CR>")
        .run()
        .await;

    // Should be back in normal mode after closing
    result.assert_normal_mode();
}

// ============================================================================
// Navigation Tests (j/k)
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_navigate_down() {
    // Create some undo history, open undotree, navigate down
    let result = IntegrationTest::new()
        .await
        .with_buffer("original")
        .send_keys("cwnew<Esc>") // Change word to "new" (creates undo point)
        .send_keys(":undotree<CR>") // Open undotree
        .send_keys("j") // Navigate down in tree
        .run()
        .await;

    // Should still be in undotree mode after navigation
    assert_undotree_mode(&result);
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_navigate_up() {
    // Create undo history, open undotree, navigate down then up
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1")
        .send_keys("oline 2<Esc>") // Add line (creates undo point)
        .send_keys(":undotree<CR>") // Open undotree
        .send_keys("jk") // Navigate down then up
        .run()
        .await;

    assert_undotree_mode(&result);
}

// ============================================================================
// Enter (Goto Selected) Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_goto_selected() {
    // Create undo history, navigate to earlier state, press Enter to apply
    let result = IntegrationTest::new()
        .await
        .with_buffer("original")
        .send_keys("cwchanged<Esc>") // Change to "changed" (node 1)
        .send_keys(":undotree<CR>") // Open undotree (at node 1)
        .send_keys("j") // Navigate to root (node 0)
        .send_keys("<CR>") // Apply - go to selected node
        .run()
        .await;

    // After applying, should restore original content
    result.assert_buffer_eq("original");
    // Should return to normal mode after applying
    result.assert_normal_mode();
}

// ============================================================================
// Close Panel Tests (q/Esc)
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_close_with_q() {
    // Open undotree, close with q
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys(":undotree<CR>") // Open undotree
        .send_keys("q") // Close with q
        .run()
        .await;

    // Should be back in normal mode
    result.assert_normal_mode();
    // Buffer should be unchanged
    result.assert_buffer_eq("hello");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_close_with_escape() {
    // Open undotree, close with Escape
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys(":undotree<CR>") // Open undotree
        .send_keys("<Esc>") // Close with Escape
        .run()
        .await;

    result.assert_normal_mode();
}

// ============================================================================
// Panel State Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_with_undo_history() {
    // Create multiple undo points, verify undotree opens correctly
    let result = IntegrationTest::new()
        .await
        .with_buffer("one")
        .send_keys("cwtwo<Esc>") // one -> two
        .send_keys("cwthree<Esc>") // two -> three
        .send_keys("cwfour<Esc>") // three -> four
        .send_keys(":undotree<CR>") // Open undotree with 4 nodes
        .run()
        .await;

    assert_undotree_mode(&result);
    // Buffer should still show latest content
    result.assert_buffer_eq("four");
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_preserves_undo_after_close() {
    // Ensure closing undotree doesn't affect undo history
    let result = IntegrationTest::new()
        .await
        .with_buffer("original")
        .send_keys("cwmodified<Esc>") // Create undo point
        .send_keys(":undotree<CR>") // Open undotree
        .send_keys("q") // Close without applying
        .send_keys("u") // Normal undo should still work
        .run()
        .await;

    result.assert_buffer_eq("original");
}

// ============================================================================
// Panel Refresh Tests
// ============================================================================

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_panel_refresh_on_undo() {
    // Open undotree panel, perform undo while panel is open
    // Panel should update to show new current node position
    let result = IntegrationTest::new()
        .await
        .with_buffer("original")
        .send_keys("cwmodified<Esc>") // Create undo point (node 1)
        .send_keys(":undotree<CR>") // Open panel (showing node 1 as current)
        .send_keys("u") // Undo while panel is open
        .run()
        .await;

    // Buffer should be restored to original
    result.assert_buffer_eq("original");
    // Panel should still be open in undotree mode
    assert_undotree_mode(&result);
}

#[tokio::test]
#[ignore = "requires modules loaded for key bindings"]
async fn test_undotree_panel_refresh_on_redo() {
    // Open undotree panel, perform undo then redo while panel is open
    let result = IntegrationTest::new()
        .await
        .with_buffer("original")
        .send_keys("cwmodified<Esc>") // Create undo point
        .send_keys(":undotree<CR>") // Open panel
        .send_keys("u") // Undo (go to node 0)
        .send_keys("<C-r>") // Redo (go back to node 1)
        .run()
        .await;

    // Buffer should show modified content after redo
    result.assert_buffer_eq("modified");
    // Panel should still be open
    assert_undotree_mode(&result);
}
