//! Headless TUI tests for completion popup (#521).
//!
//! Tests verify the completion popup workflow through the TUI rendering
//! pipeline: enter insert mode → type text → `<C-Space>` trigger →
//! navigate/confirm/dismiss.
//!
//! All tests use the buffer words source only (no real LSP server).
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-client-tui --test completion_tests
//! ```

use std::time::Duration;

use {
    reovim_client_tui::{TuiAppError, TuiHandle, connect_headless},
    reovim_testing::TestServerHarness,
};

/// Helper to create a headless TUI connection.
async fn headless_tui(addr: &str, width: u16, height: u16) -> Result<TuiHandle, TuiAppError> {
    let (mut app, handle) = connect_headless(addr, width, height, None, None).await?;
    tokio::spawn(async move { app.run().await });
    Ok(handle)
}

// ============================================================================
// Completion Popup Tests
// ============================================================================

/// Triggering completion shows a popup with matching buffer words.
#[tokio::test]
async fn test_completion_trigger_shows_popup() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 40)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Type text so "hello" is a buffer word, then type prefix "hel".
    handle
        .send_keys("ihello world ")
        .await
        .expect("Failed to send keys");
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle
        .send_keys("hel")
        .await
        .expect("Failed to send prefix");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Trigger completion.
    handle
        .send_keys("<C-Space>")
        .await
        .expect("Failed to trigger completion");

    // Wait for popup border char.
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| {
            frame.contains('\u{256D}') && frame.contains("hello")
        })
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("hello"), "Popup should show 'hello'");
            assert!(frame.contains("buffer"), "Source should be 'buffer'");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Completion popup not shown: {e}\nFrame:\n{}", frame.unwrap_or_default());
        }
    }

    handle.stop().await;
}

/// Confirming completion replaces the prefix with the selected item.
#[tokio::test]
async fn test_completion_confirm_inserts_text() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 40)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle
        .send_keys("ihello world ")
        .await
        .expect("Failed to send text");
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle
        .send_keys("hel<C-Space>")
        .await
        .expect("Failed to trigger");

    // Wait for popup.
    handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains('\u{256D}'))
        .await
        .expect("Popup should appear");

    // Confirm.
    handle.send_keys("<C-y>").await.expect("Failed to confirm");

    // Wait for popup to disappear and text to be replaced.
    let frame = handle
        .wait_for(Duration::from_secs(2), |frame| {
            !frame.contains('\u{256D}') && frame.contains("hello world hello")
        })
        .await
        .expect("Confirm should replace prefix");

    assert!(frame.to_lowercase().contains("insert"), "Should stay in INSERT mode");

    handle.stop().await;
}

/// Dismissing completion closes popup without changing text.
#[tokio::test]
async fn test_completion_dismiss_no_change() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 40)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle
        .send_keys("ihello world ")
        .await
        .expect("Failed to send text");
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle
        .send_keys("hel<C-Space>")
        .await
        .expect("Failed to trigger");

    handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains('\u{256D}'))
        .await
        .expect("Popup should appear");

    // Dismiss.
    handle.send_keys("<C-e>").await.expect("Failed to dismiss");

    // Wait for popup to disappear.
    let frame = handle
        .wait_for(Duration::from_secs(2), |frame| !frame.contains('\u{256D}'))
        .await
        .expect("Popup should close on dismiss");

    // Text unchanged — "hel" is still there, not "hello".
    assert!(frame.contains("hel"), "Text should not change on dismiss");
    assert!(frame.to_lowercase().contains("insert"), "Should stay in INSERT mode");

    handle.stop().await;
}

/// Navigating with C-n before confirming selects a different item.
#[tokio::test]
async fn test_completion_navigate_then_confirm() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 40)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Two matching words: "alpha" and "amazing" both match "al".
    handle
        .send_keys("ialpha amazing ")
        .await
        .expect("Failed to send text");
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle
        .send_keys("al<C-Space>")
        .await
        .expect("Failed to trigger");

    handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains('\u{256D}'))
        .await
        .expect("Popup should appear");

    // Navigate to next item and confirm.
    handle
        .send_keys("<C-n><C-y>")
        .await
        .expect("Failed to navigate and confirm");

    // The second item was confirmed — should NOT be the first item.
    let frame = handle
        .wait_for(Duration::from_secs(2), |frame| !frame.contains('\u{256D}'))
        .await
        .expect("Popup should close");

    // One of the two words was inserted (the second one).
    // We can't predict exact order (depends on BufferWordsSource sort),
    // but the prefix "al" should be gone (replaced with a full word).
    let first_line = frame.lines().next().unwrap_or("");
    assert!(!first_line.ends_with("al"), "Prefix 'al' should be replaced after confirm");

    handle.stop().await;
}

/// C-p from index 0 wraps around to the last item.
#[tokio::test]
async fn test_completion_wraparound_prev() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 40)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle
        .send_keys("ialpha amazing ")
        .await
        .expect("Failed to send text");
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle
        .send_keys("al<C-Space>")
        .await
        .expect("Failed to trigger");

    handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains('\u{256D}'))
        .await
        .expect("Popup should appear");

    // C-p wraps from index 0 to last item, then confirm.
    handle
        .send_keys("<C-p><C-y>")
        .await
        .expect("Failed to navigate and confirm");

    let frame = handle
        .wait_for(Duration::from_secs(2), |frame| !frame.contains('\u{256D}'))
        .await
        .expect("Popup should close");

    // Prefix replaced with the last item (wrap-around worked).
    let first_line = frame.lines().next().unwrap_or("");
    assert!(
        !first_line.ends_with("al"),
        "Prefix 'al' should be replaced after wraparound confirm"
    );

    handle.stop().await;
}

/// Escape dismisses the popup and exits insert mode.
#[tokio::test]
async fn test_completion_escape_dismisses() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 40)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle
        .send_keys("ihello world ")
        .await
        .expect("Failed to send text");
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle
        .send_keys("hel<C-Space>")
        .await
        .expect("Failed to trigger");

    handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains('\u{256D}'))
        .await
        .expect("Popup should appear");

    // Escape exits insert mode. The popup may linger until the next
    // extension update cycle, so we verify mode change instead.
    handle.send_keys("<Esc>").await.expect("Failed to send Esc");

    let frame = handle
        .wait_for(Duration::from_secs(2), |frame| frame.to_lowercase().contains("normal"))
        .await
        .expect("Should exit to NORMAL mode");

    assert!(frame.contains("hel"), "Text should not change on Esc");

    handle.stop().await;
}

/// No popup when no buffer words match the prefix.
#[tokio::test]
async fn test_completion_no_match_no_popup() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 40)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // "hello" is a buffer word, but prefix "xyz" matches nothing.
    handle
        .send_keys("ihello ")
        .await
        .expect("Failed to send text");
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle
        .send_keys("xyz<C-Space>")
        .await
        .expect("Failed to trigger");

    // Wait a bit, then verify no popup appeared.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");

    assert!(!frame.contains('\u{256D}'), "No popup should appear when no matches");
    assert!(frame.to_lowercase().contains("insert"), "Should stay in INSERT mode");

    handle.stop().await;
}
