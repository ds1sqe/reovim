//! Integration tests for the notification plugin
//!
//! Tests the notification plugin's core functionality including:
//! - Showing notifications via RPC
//! - Progress updates via RPC
//! - Notification state queries
//! - Visual rendering of notifications

use reovim_core::testing::ServerTest;

/// Test that notification state starts empty
#[tokio::test]
async fn test_notification_starts_empty() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .run()
        .await;

    result.assert_notification_hidden();
    result.assert_notification_count(0);
    result.assert_progress_count(0);
}

/// Test showing an info notification via RPC
#[tokio::test]
async fn test_show_info_notification() {
    let mut result = ServerTest::new()
        .await
        .with_content("test content")
        .run()
        .await;

    // Show a notification
    result.show_notification("Test info message", "info").await;
    result.refresh_notification().await;

    result.assert_notification_visible();
    result.assert_notification_count(1);
    result.assert_notification_message("Test info message");
    result.assert_notification_level("Info");
}

/// Test showing a success notification via RPC
#[tokio::test]
async fn test_show_success_notification() {
    let mut result = ServerTest::new()
        .await
        .with_content("test content")
        .run()
        .await;

    result
        .show_notification("Operation completed", "success")
        .await;
    result.refresh_notification().await;

    result.assert_notification_visible();
    result.assert_notification_level("Success");
}

/// Test showing a warning notification via RPC
#[tokio::test]
async fn test_show_warning_notification() {
    let mut result = ServerTest::new()
        .await
        .with_content("test content")
        .run()
        .await;

    result
        .show_notification("Something may be wrong", "warning")
        .await;
    result.refresh_notification().await;

    result.assert_notification_visible();
    result.assert_notification_level("Warning");
}

/// Test showing an error notification via RPC
#[tokio::test]
async fn test_show_error_notification() {
    let mut result = ServerTest::new()
        .await
        .with_content("test content")
        .run()
        .await;

    result.show_notification("An error occurred", "error").await;
    result.refresh_notification().await;

    result.assert_notification_visible();
    result.assert_notification_level("Error");
}

/// Test showing multiple notifications
#[tokio::test]
async fn test_multiple_notifications() {
    let mut result = ServerTest::new()
        .await
        .with_content("test content")
        .run()
        .await;

    result.show_notification("First message", "info").await;
    result.show_notification("Second message", "success").await;
    result.show_notification("Third message", "warning").await;
    result.refresh_notification().await;

    result.assert_notification_visible();
    result.assert_notification_count(3);
    result.assert_notification_message("First message");
    result.assert_notification_message("Second message");
    result.assert_notification_message("Third message");
}

/// Test progress update via RPC
#[tokio::test]
async fn test_progress_update() {
    let mut result = ServerTest::new()
        .await
        .with_content("test content")
        .run()
        .await;

    result
        .update_progress("build", "Building project", "cargo", Some(50))
        .await;
    result.refresh_notification().await;

    result.assert_notification_visible();
    result.assert_progress_count(1);
    result.assert_progress_title("Building project");
}

/// Test progress without percentage (indeterminate)
#[tokio::test]
async fn test_progress_indeterminate() {
    let mut result = ServerTest::new()
        .await
        .with_content("test content")
        .run()
        .await;

    result
        .update_progress("lsp", "Loading language server", "rust-analyzer", None)
        .await;
    result.refresh_notification().await;

    result.assert_notification_visible();
    result.assert_progress_count(1);
    result.assert_progress_title("Loading language server");
}

/// Test multiple progress items
#[tokio::test]
async fn test_multiple_progress() {
    let mut result = ServerTest::new()
        .await
        .with_content("test content")
        .run()
        .await;

    result
        .update_progress("build", "Building", "cargo", Some(25))
        .await;
    result
        .update_progress("test", "Running tests", "cargo", Some(75))
        .await;
    result.refresh_notification().await;

    result.assert_notification_visible();
    result.assert_progress_count(2);
    result.assert_progress_title("Building");
    result.assert_progress_title("Running tests");
}

/// Test notification state changes after showing
///
/// Verifies that the notification manager's state is correctly updated
/// when notifications are shown via RPC. The state correctly reports
/// `has_visible=true` after a notification is added.
#[tokio::test]
async fn test_notification_state_after_show() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("test content")
        .run()
        .await;

    // Initially no notifications
    result.refresh_notification().await;
    result.assert_notification_hidden();

    // Show notification via RPC
    result.show_notification("Test message", "info").await;
    result.refresh_notification().await;

    // Verify state is correctly updated
    result.assert_notification_visible();
    result.assert_notification_count(1);
    result.assert_notification_message("Test message");
    result.assert_notification_level("Info");
}

/// Test progress state changes after updating
///
/// Verifies that progress notifications are correctly tracked in the
/// manager's state after being added via RPC.
#[tokio::test]
async fn test_progress_state_after_update() {
    let mut result = ServerTest::new()
        .await
        .with_size(80, 24)
        .with_content("test content")
        .run()
        .await;

    // Initially no progress
    result.refresh_notification().await;
    result.assert_progress_count(0);

    // Update progress via RPC
    result
        .update_progress("test", "Test Progress", "test", Some(50))
        .await;
    result.refresh_notification().await;

    // Verify state is correctly updated
    result.assert_notification_visible();
    result.assert_progress_count(1);
    result.assert_progress_title("Test Progress");
}
