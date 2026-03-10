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
    // #451: Cmdline is now a floating popup with box-drawing borders.
    // Look for the popup content row containing "│" and the ":" prompt.
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| {
            frame.lines().any(|l| l.contains('│') && l.contains(':'))
        })
        .await;

    match result {
        Ok(frame) => {
            let content_line = frame
                .lines()
                .find(|l| l.contains('│') && l.contains(':'))
                .unwrap_or("");
            assert!(
                content_line.contains(':'),
                "Popup content row should contain ':' prompt, got: '{content_line}'"
            );
            eprintln!("[test] Cmdline popup activated: '{content_line}'");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Cmdline did not activate: {e}\nFrame:\n{}", frame.unwrap_or_default());
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

    // #451: Wait for "wq" to appear inside the floating popup content row.
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| {
            frame.lines().any(|l| l.contains('│') && l.contains("wq"))
        })
        .await;

    match result {
        Ok(frame) => {
            let content_line = frame
                .lines()
                .find(|l| l.contains('│') && l.contains("wq"))
                .unwrap_or("");
            assert!(content_line.contains("wq"), "Popup should show ':wq', got: '{content_line}'");
            eprintln!("[test] Cmdline input visible: '{content_line}'");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Cmdline input not visible: {e}\nFrame:\n{}", frame.unwrap_or_default());
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

    // #451: Verify popup is visible (box-drawing border present)
    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");
    assert!(frame.contains('╭'), "Popup border should be visible before Escape");
    eprintln!("[test] Before Escape - popup visible");

    // Press Escape to deactivate
    handle
        .send_keys("<Esc>")
        .await
        .expect("Failed to send Escape");

    // #451: Wait for popup to disappear (no more box-drawing borders)
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| !frame.contains('╭'))
        .await;

    match result {
        Ok(frame) => {
            assert!(!frame.contains('╭'), "Popup should be gone after Escape");
            eprintln!("[test] After Escape - popup gone");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Cmdline did not deactivate: {e}\nFrame:\n{}", frame.unwrap_or_default());
        }
    }

    handle.stop().await;
}

// ============================================================================
// Range-Finder Module Tests (#524)
// ============================================================================

/// Test that pressing `s` (jump search) does not crash and stays in normal mode.
///
/// The command `execute()` is a stub, so `s` should dispatch the command
/// and return to normal mode without visible change.
#[tokio::test]
async fn test_range_finder_s_key_no_crash() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Type some text first
    handle
        .send_keys("ihello world<Esc>")
        .await
        .expect("Failed to send keys");

    // Wait for text
    handle
        .wait_for(Duration::from_secs(2), |f| f.contains("hello"))
        .await
        .expect("Text should appear");

    // Press `s` (jump search) - should not crash, stays in normal
    handle.send_keys("s").await.expect("Failed to send s");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify server still responds and text is intact
    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture after s");
    assert!(frame.contains("hello"), "Buffer should still contain text after s key");

    handle.stop().await;
}

/// Test that fold keys (`za`, `zo`, `zc`, `zR`, `zM`) don't crash the TUI.
///
/// All fold commands are stubs, so they should dispatch and return
/// without visible change. Tests that the `z` prefix is handled correctly.
#[tokio::test]
async fn test_range_finder_fold_keys_no_crash() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Type some multi-line text
    handle
        .send_keys("ifn main() {<CR>    println!(\"hello\");<CR>}<Esc>")
        .await
        .expect("Failed to send keys");

    handle
        .wait_for(Duration::from_secs(2), |f| f.contains("main"))
        .await
        .expect("Text should appear");

    // Test each fold key - none should crash
    for keys in &["za", "zo", "zc", "zR", "zM"] {
        handle
            .send_keys(keys)
            .await
            .unwrap_or_else(|_| panic!("Failed to send {keys}"));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Verify server still responds
    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture after fold keys");
    assert!(frame.contains("main"), "Buffer should still contain text after fold keys");

    handle.stop().await;
}

/// Test that `s` key followed by Escape recovers to normal mode.
///
/// When the `execute()` is wired, `s` will enter jump-input mode.
/// For now (stub), it just returns Success and stays in normal mode.
#[tokio::test]
async fn test_range_finder_s_then_escape() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Press s then Escape
    handle.send_keys("s").await.expect("Failed to send s");
    tokio::time::sleep(Duration::from_millis(50)).await;

    handle
        .send_keys("<Esc>")
        .await
        .expect("Failed to send Escape");

    // Should be in normal mode
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| {
            let lower = frame.to_lowercase();
            lower.contains("normal") || !lower.contains("insert")
        })
        .await;

    assert!(result.is_ok(), "Should be in normal mode after s + Escape");

    handle.stop().await;
}

// ============================================================================
// Microscope Picker Tests (#522)
// ============================================================================

/// Test that `<Space>f` opens the file picker with the prompt and title visible.
#[tokio::test]
async fn test_picker_opens_on_space_f() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send <Space>f to open file picker
    handle
        .send_keys("<Space>f")
        .await
        .expect("Failed to send keys");

    // Wait for the picker prompt "> " to appear in the frame
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains("> "))
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("> "), "Frame should show picker prompt '> '");
            eprintln!("[test] Picker opened with prompt visible");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Picker did not open: {e}\nFrame:\n{}", frame.unwrap_or_default());
        }
    }

    handle.stop().await;
}

/// Test that typing in the picker updates the query display.
#[tokio::test]
async fn test_picker_query_input() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Open file picker and type a query
    handle
        .send_keys("<Space>f")
        .await
        .expect("Failed to send keys");
    tokio::time::sleep(Duration::from_millis(200)).await;

    handle.send_keys("m").await.expect("Failed to send m");
    handle.send_keys("a").await.expect("Failed to send a");
    handle.send_keys("i").await.expect("Failed to send i");
    handle.send_keys("n").await.expect("Failed to send n");

    // Wait for "main" to appear in the frame (query display)
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains("main"))
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("main"), "Frame should show typed query 'main'");
            eprintln!("[test] Picker query 'main' visible");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Picker query not visible: {e}\nFrame:\n{}", frame.unwrap_or_default());
        }
    }

    handle.stop().await;
}

/// Test that Escape closes the picker.
#[tokio::test]
async fn test_picker_closes_on_escape() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Open file picker
    handle
        .send_keys("<Space>f")
        .await
        .expect("Failed to send keys");

    // Wait for picker to open
    let _ = handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains("> "))
        .await
        .expect("Picker did not open");

    // Close picker with Escape
    handle
        .send_keys("<Esc>")
        .await
        .expect("Failed to send Escape");

    // Wait for mode to return to NORMAL (no more MICROSCOPE)
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| {
            let lower = frame.to_lowercase();
            lower.contains("normal") && !lower.contains("microscope")
        })
        .await;

    match result {
        Ok(frame) => {
            assert!(
                frame.to_lowercase().contains("normal"),
                "Should return to NORMAL mode after closing picker"
            );
            eprintln!("[test] Picker closed, returned to NORMAL");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Picker did not close: {e}\nFrame:\n{}", frame.unwrap_or_default());
        }
    }

    handle.stop().await;
}

/// Test that backspace removes characters from the picker query.
#[tokio::test]
async fn test_picker_backspace_removes_char() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Open file picker and type "ab"
    handle
        .send_keys("<Space>f")
        .await
        .expect("Failed to send keys");
    tokio::time::sleep(Duration::from_millis(200)).await;

    handle.send_keys("a").await.expect("Failed to send a");
    handle.send_keys("b").await.expect("Failed to send b");

    // Wait for "ab" to appear
    let _ = handle
        .wait_for(Duration::from_secs(2), |frame| frame.contains("ab"))
        .await
        .expect("Query 'ab' not visible");

    // Press backspace to remove 'b'
    handle.send_keys("<BS>").await.expect("Failed to send BS");

    // Wait for query line to show "> a" without "b" (the prompt line specifically).
    // We check that no line starts with "> ab" (the query line), rather than
    // checking the entire frame, since item file paths may contain "ab".
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| {
            frame
                .lines()
                .any(|line| line.starts_with("> a") && !line.starts_with("> ab"))
        })
        .await;

    match result {
        Ok(_) => {
            eprintln!("[test] Backspace removed character from query");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Backspace did not remove character: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

/// Test that `<Space>g` opens the grep picker with the "rg> " prompt.
#[tokio::test]
async fn test_grep_picker_opens() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Open grep picker
    handle
        .send_keys("<Space>g")
        .await
        .expect("Failed to send keys");

    // Wait for the grep prompt "rg> " to appear
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains("rg>"))
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("rg>"), "Frame should show grep prompt 'rg> '");
            eprintln!("[test] Grep picker opened with rg> prompt");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Grep picker did not open: {e}\nFrame:\n{}", frame.unwrap_or_default());
        }
    }

    // Close with Escape
    handle.send_keys("<Esc>").await.expect("Failed to close");
    handle.stop().await;
}

/// Test that `<Space>b` opens the buffer picker.
#[tokio::test]
async fn test_buffer_picker_opens() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Open buffer picker
    handle
        .send_keys("<Space>b")
        .await
        .expect("Failed to send keys");

    // Wait for the picker prompt (buffer picker also uses "> ")
    // Note: the picker overlay covers the statusline, so MICROSCOPE mode text
    // is not visible. Check for the picker prompt and count indicator instead.
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains("> ") && frame.contains('['))
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("> "), "Frame should show buffer picker prompt");
            eprintln!("[test] Buffer picker opened");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Buffer picker did not open: {e}\nFrame:\n{}", frame.unwrap_or_default());
        }
    }

    // Close with Escape
    handle.send_keys("<Esc>").await.expect("Failed to close");
    handle.stop().await;
}

/// Test that `<Space>;` opens the command picker.
#[tokio::test]
async fn test_command_picker_opens() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Open command picker
    handle
        .send_keys("<Space>;")
        .await
        .expect("Failed to send keys");

    // Wait for picker prompt to appear (overlay covers statusline, so check
    // for the prompt and count indicator instead of mode text).
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains("> ") && frame.contains('['))
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("> "), "Frame should show command picker prompt");
            eprintln!("[test] Command picker opened");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Command picker did not open: {e}\nFrame:\n{}", frame.unwrap_or_default());
        }
    }

    // Close with Escape
    handle.send_keys("<Esc>").await.expect("Failed to close");
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

    // #451: Search cmdline now renders as floating popup with '/' prompt.
    let result = handle
        .wait_for(Duration::from_secs(2), |frame| {
            frame.lines().any(|l| l.contains('│') && l.contains('/'))
        })
        .await;

    match result {
        Ok(frame) => {
            let content_line = frame
                .lines()
                .find(|l| l.contains('│') && l.contains('/'))
                .unwrap_or("");
            assert!(
                content_line.contains('/'),
                "Popup should show '/' search prompt, got: '{content_line}'"
            );
            eprintln!("[test] Search cmdline popup visible: '{content_line}'");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!("Search cmdline not visible: {e}\nFrame:\n{}", frame.unwrap_or_default());
        }
    }

    handle.stop().await;
}
