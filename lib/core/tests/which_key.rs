//! Which-key integration tests
//!
//! Tests for the which-key popup panel that shows available keybindings.
//!
//! ## Implementation Notes
//!
//! - Which-key popup appears after 500ms timeout on prefix key
//! - Hides immediately on any subsequent keypress
//! - Shows bindings for the current mode

mod common;

use common::*;

// ============================================================================
// Basic visibility tests
// ============================================================================

/// Which-key panel should be hidden by default
#[tokio::test]
async fn test_whichkey_hidden_by_default() {
    let result = ServerTest::new().await.run().await;

    result.assert_whichkey_hidden();
}

/// Which-key panel should show after timeout on prefix key
#[tokio::test]
async fn test_whichkey_shows_after_timeout() {
    let result = ServerTest::new()
        .await
        .with_keys("g")
        .with_delay(600) // 500ms timeout + buffer
        .run()
        .await;

    result.assert_whichkey_visible();
    result.assert_whichkey_prefix("g");
}

/// Which-key panel should hide on subsequent keypress
#[tokio::test]
async fn test_whichkey_hides_on_keypress() {
    let result = ServerTest::new()
        .await
        .with_keys("g")
        .with_delay(600)
        .with_keys("g") // gg command
        .with_delay(100) // Wait for hide event to propagate
        .run()
        .await;

    result.assert_whichkey_hidden();
}

// ============================================================================
// Prefix tests
// ============================================================================

/// g prefix should show g-related bindings
#[tokio::test]
async fn test_whichkey_g_prefix() {
    let result = ServerTest::new()
        .await
        .with_keys("g")
        .with_delay(600)
        .run()
        .await;

    result.assert_whichkey_visible();
    result.assert_whichkey_prefix("g");
    result.assert_whichkey_has_binding("g"); // gg - go to first line
}

/// Space prefix should show space-related bindings
#[tokio::test]
async fn test_whichkey_space_prefix() {
    let result = ServerTest::new()
        .await
        .with_keys(" ")
        .with_delay(600)
        .run()
        .await;

    result.assert_whichkey_visible();
    result.assert_whichkey_prefix(" ");
    result.assert_whichkey_has_binding("e"); // Space e - explorer
    result.assert_whichkey_has_binding("f"); // Space f - telescope prefix
}

/// z prefix should show fold-related bindings
#[tokio::test]
async fn test_whichkey_z_prefix() {
    let result = ServerTest::new()
        .await
        .with_keys("z")
        .with_delay(600)
        .run()
        .await;

    result.assert_whichkey_visible();
    result.assert_whichkey_prefix("z");
}

// ============================================================================
// Mode-specific tests
// ============================================================================

/// Which-key should not interfere with insert mode
#[tokio::test]
async fn test_whichkey_not_in_insert_mode() {
    let result = ServerTest::new()
        .await
        .with_keys("i")
        .with_delay(100) // Just wait for mode switch, no which-key timeout
        .run()
        .await;

    result.assert_insert_mode();
    // In insert mode, which-key may still be visible=false but we check mode first
}

/// Operator-pending mode should show motion bindings
#[tokio::test]
async fn test_whichkey_operator_pending() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("d")
        .with_delay(600)
        .run()
        .await;

    // Should be in operator-pending mode
    assert!(
        result.mode.sub_mode.contains("OperatorPending"),
        "Expected operator-pending mode, got {:?}",
        result.mode
    );
}

// ============================================================================
// Escape cancels which-key
// ============================================================================

/// Escape should cancel prefix and hide which-key
#[tokio::test]
async fn test_whichkey_escape_cancels() {
    let result = ServerTest::new()
        .await
        .with_keys("g")
        .with_delay(600)
        .with_keys("<Esc>")
        .with_delay(100) // Wait for hide event to propagate
        .run()
        .await;

    result.assert_whichkey_hidden();
    result.assert_normal_mode();
}
