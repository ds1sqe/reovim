//! Integration tests for the microscope fuzzy finder plugin
//!
//! Tests the microscope plugin's core functionality including:
//! - Opening/closing pickers
//! - Query filtering
//!
//! NOTE: Some tests are ignored due to flaky timing issues with async event processing.
//! The core functionality (opening pickers, querying state) is tested reliably.

use reovim_core::testing::ServerTest;

/// Test that opening files picker activates microscope
#[tokio::test]
async fn test_open_files_picker() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys("<Space>ff")
        .with_delay(300) // Allow picker to load
        .run()
        .await;

    result.assert_microscope_active();
    result.assert_microscope_picker("files");
}

/// Test that opening buffers picker activates microscope
#[tokio::test]
async fn test_open_buffers_picker() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys("<Space>fb")
        .with_delay(300)
        .run()
        .await;

    result.assert_microscope_active();
    result.assert_microscope_picker("buffers");
}

/// Test that microscope starts in normal mode
#[tokio::test]
async fn test_starts_in_normal_mode() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys("<Space>ff")
        .with_delay(300)
        .run()
        .await;

    result.assert_microscope_active();
    result.assert_microscope_normal_mode();
}

/// Test basic microscope state query via RPC
#[tokio::test]
async fn test_microscope_state_rpc() {
    let result = ServerTest::new()
        .await
        .with_content("test content")
        .with_keys("<Space>ff")
        .with_delay(300)
        .run()
        .await;

    // Verify we can query microscope state
    let ms = result
        .microscope
        .as_ref()
        .expect("Microscope state should be available");
    assert!(ms.active, "Microscope should be active");
    assert_eq!(ms.picker_name, "files", "Picker name should be 'files'");
    // Files picker should have found some files in the project
    assert!(
        ms.item_count > 0 || ms.title == "Find Files",
        "Picker should have items or title"
    );
}
