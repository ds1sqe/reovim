//! TUI E2E tests using headless mode and frame capture.
//!
//! These tests verify the TUI client renders correctly by:
//! 1. Spawning a test server
//! 2. Connecting a headless TUI
//! 3. Sending keys and capturing frames
//! 4. Asserting on frame content
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-client-tui --test headless_tests
//! ```

use std::time::Duration;

use {
    reovim_client_tui::{TuiAppError, TuiHandle, connect_headless},
    reovim_testing::TestServerHarness,
};

/// Helper to create a headless TUI connection.
///
/// Connects to the server, spawns the event loop, and returns a handle.
async fn headless_tui(addr: &str, width: u16, height: u16) -> Result<TuiHandle, TuiAppError> {
    let (mut app, handle) = connect_headless(addr, width, height, None, None).await?;
    // Spawn the event loop in the background
    tokio::spawn(async move { app.run().await });
    Ok(handle)
}

// ============================================================================
// Connection Tests
// ============================================================================

/// Test that headless TUI connects successfully to a server.
#[tokio::test]
async fn test_headless_tui_connects() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    // Connect headless TUI
    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    // Small delay for initialization
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Capture frame directly from TUI
    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");

    // Verify non-empty frame - should have at least tildes for empty buffer
    assert!(!frame.is_empty(), "Frame should have content");

    handle.stop().await;
}

/// Test that TUI renders the statusline with mode indicator.
#[tokio::test]
async fn test_statusline_renders() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");

    // Statusline should be at the bottom and contain mode indicator
    let last_line = frame.lines().last().unwrap_or("");
    assert!(!last_line.is_empty(), "Statusline (last line) should not be empty");

    handle.stop().await;
}

// ============================================================================
// Mode Indicator Tests (Require Module Loading)
// ============================================================================

/// Test that entering insert mode shows INSERT in the frame.
///
/// Requires vim module to be loaded for 'i' keybinding to work.
#[tokio::test]
async fn test_insert_mode_indicator() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send 'i' to enter insert mode
    handle.send_keys("i").await.expect("Failed to send keys");

    // Wait for mode to change
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| frame.to_lowercase().contains("insert"))
        .await;

    match result {
        Ok(frame) => {
            assert!(
                frame.to_lowercase().contains("insert"),
                "Frame should show INSERT mode after pressing 'i'"
            );
        }
        Err(e) => {
            // If wait_for times out, capture current frame for debugging
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Failed to wait for INSERT mode: {e}\nCurrent frame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

/// Test that pressing Escape returns to normal mode.
///
/// Note: This test passes even without module loading because the mode
/// starts as NORMAL and the predicate accepts !contains("insert").
#[tokio::test]
async fn test_escape_returns_to_normal() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Enter insert mode then escape
    handle.send_keys("i").await.expect("Failed to send 'i'");
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle
        .send_keys("<Esc>")
        .await
        .expect("Failed to send Escape");

    // Wait for mode to change back to normal
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| {
            let lower = frame.to_lowercase();
            lower.contains("normal") || !lower.contains("insert")
        })
        .await;

    match result {
        Ok(frame) => {
            assert!(
                !frame.to_lowercase().contains("insert"),
                "Frame should not show INSERT mode after Escape"
            );
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Failed to return to normal mode: {e}\nCurrent frame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

// ============================================================================
// Text Input Tests (Require Module Loading)
// ============================================================================

/// Test that typing in insert mode shows text in the buffer.
///
/// Requires vim module to be loaded for 'i' keybinding and text insertion.
#[tokio::test]
async fn test_text_input_visible() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Enter insert mode and type "hello"
    handle
        .send_keys("ihello<Esc>")
        .await
        .expect("Failed to send keys");

    // Wait for text to appear
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| frame.contains("hello"))
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("hello"), "Frame should contain typed text 'hello'");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Failed to find typed text: {e}\nCurrent frame:\n{}", frame.unwrap_or_default());
        }
    }

    handle.stop().await;
}

// ============================================================================
// Resize Tests
// ============================================================================

/// Test that resizing the TUI updates the viewport.
#[tokio::test]
async fn test_resize_updates_viewport() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Resize to larger dimensions
    handle.resize(120, 40).await.expect("Failed to resize");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Capture after resize
    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");

    // The frame should be non-empty (we can't easily verify dimensions in plain text)
    assert!(!frame.is_empty(), "Frame should have content after resize");

    handle.stop().await;
}

// ============================================================================
// Cmdline UI Tests (#469)
// ============================================================================

/// Test that pressing `:` activates the cmdline bar and it appears in the frame.
#[tokio::test]
async fn test_cmdline_activates_on_colon() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send `:` to activate cmdline
    handle.send_keys(":").await.expect("Failed to send :");

    // Wait for cmdline prompt to appear on the second-to-last row
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| {
            let lines: Vec<&str> = frame.lines().collect();
            let row = lines.len().saturating_sub(2);
            lines.get(row).is_some_and(|l| l.starts_with(':'))
        })
        .await;

    match result {
        Ok(frame) => {
            let lines: Vec<&str> = frame.lines().collect();
            let cmdline_row = lines.len().saturating_sub(2);
            let cmdline_line = lines.get(cmdline_row).unwrap_or(&"");
            assert!(
                cmdline_line.starts_with(':'),
                "Cmdline row should start with ':' prompt, got: '{cmdline_line}'"
            );
            eprintln!("[test] Cmdline activated. Row {cmdline_row}: '{cmdline_line}'");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Cmdline did not activate: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

/// Test that typing in cmdline mode shows input text in real-time.
#[tokio::test]
async fn test_cmdline_typing_visible() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Activate cmdline and type "wq"
    handle.send_keys(":").await.expect("Failed to send :");
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.send_keys("w").await.expect("Failed to send w");
    handle.send_keys("q").await.expect("Failed to send q");

    // Wait for "wq" to appear
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| frame.contains("wq"))
        .await;

    match result {
        Ok(frame) => {
            let lines: Vec<&str> = frame.lines().collect();
            let cmdline_row = lines.len().saturating_sub(2);
            let cmdline_line = lines.get(cmdline_row).unwrap_or(&"");
            assert!(
                cmdline_line.contains("wq"),
                "Cmdline should show ':wq', got: '{cmdline_line}'"
            );
            eprintln!("[test] Cmdline input visible. Row {cmdline_row}: '{cmdline_line}'");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Cmdline input not visible: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

/// Test that Escape deactivates the cmdline bar.
#[tokio::test]
async fn test_cmdline_deactivates_on_escape() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Activate cmdline
    handle.send_keys(":").await.expect("Failed to send :");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify cmdline is active
    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");
    let lines: Vec<&str> = frame.lines().collect();
    let cmdline_row = lines.len().saturating_sub(2);
    let cmdline_line = lines.get(cmdline_row).unwrap_or(&"");
    eprintln!("[test] Before Escape - cmdline row: '{cmdline_line}'");

    // Press Escape to deactivate
    handle
        .send_keys("<Esc>")
        .await
        .expect("Failed to send Escape");

    // Wait for cmdline to disappear (row should no longer start with ':')
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| {
            let lines: Vec<&str> = frame.lines().collect();
            let row = lines.len().saturating_sub(2);
            let line = lines.get(row).unwrap_or(&"");
            // After Escape, cmdline row should NOT show ':' prompt
            !line.starts_with(':')
        })
        .await;

    match result {
        Ok(frame) => {
            let lines: Vec<&str> = frame.lines().collect();
            let cmdline_row = lines.len().saturating_sub(2);
            let cmdline_line = lines.get(cmdline_row).unwrap_or(&"");
            eprintln!("[test] After Escape - cmdline row: '{cmdline_line}'");
            assert!(
                !cmdline_line.starts_with(':'),
                "Cmdline should be gone after Escape, got: '{cmdline_line}'"
            );
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Cmdline did not deactivate: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

/// Test that `/` activates search cmdline with `/` prompt.
#[tokio::test]
async fn test_cmdline_search_prompt() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send `/` for search
    handle.send_keys("/").await.expect("Failed to send /");

    let result = handle
        .wait_for(Duration::from_secs(2), |frame| {
            let lines: Vec<&str> = frame.lines().collect();
            let row = lines.len().saturating_sub(2);
            let line = lines.get(row).unwrap_or(&"");
            line.starts_with('/')
        })
        .await;

    match result {
        Ok(frame) => {
            let lines: Vec<&str> = frame.lines().collect();
            let cmdline_row = lines.len().saturating_sub(2);
            let cmdline_line = lines.get(cmdline_row).unwrap_or(&"");
            assert!(
                cmdline_line.starts_with('/'),
                "Search cmdline should start with '/', got: '{cmdline_line}'"
            );
            eprintln!("[test] Search cmdline visible. Row {cmdline_row}: '{cmdline_line}'");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Search cmdline not visible: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}
