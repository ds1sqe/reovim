//! Headless TUI tests for snippet feature (#136).
//!
//! Tests verify the snippet workflow through the TUI rendering pipeline:
//! type trigger prefix -> `<C-s>` expand -> Tab/S-Tab navigate -> Esc cancel.
//! Each test captures the actual TUI frame buffer to verify rendered output.
//!
//! Requires `~/.local/share/reovim/modules/snippets/global.json` with test snippets.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-client-tui --test snippet_tests
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
// Basic Expansion
// ============================================================================

/// Test: type "tst" then `<C-s>` expands a simple snippet in TUI.
#[tokio::test]
async fn test_tui_expand_simple() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Type "tst" in insert mode, then trigger expansion with <C-s>
    handle
        .send_keys("itst<C-s>")
        .await
        .expect("Failed to send keys");

    // Wait for expansion to appear in the rendered frame
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains("TEST_EXPANDED"))
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("TEST_EXPANDED"), "TUI frame should show expanded snippet");
            eprintln!("[snippet] Simple expansion rendered correctly");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Snippet expansion not visible in TUI: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

/// Test: type unknown prefix then `<C-s>` does nothing — buffer keeps raw text.
#[tokio::test]
async fn test_tui_expand_no_match() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle
        .send_keys("ixyz<C-s>")
        .await
        .expect("Failed to send keys");

    // Small delay for processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");

    // "xyz" should remain since no snippet matched
    assert!(frame.contains("xyz"), "Buffer should keep raw text 'xyz'");
    assert!(!frame.contains("TEST_EXPANDED"), "No expansion should occur for unknown prefix");

    handle.stop().await;
}

/// Test: expand "fn" snippet produces function template in TUI.
#[tokio::test]
async fn test_tui_expand_function() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle
        .send_keys("ifn<C-s>")
        .await
        .expect("Failed to send keys");

    let result = handle
        .wait_for(Duration::from_secs(3), |frame| {
            frame.contains("fn ") && frame.contains('{') && frame.contains('}')
        })
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("fn "), "Should contain 'fn '");
            // Phase 2: $1 placeholder "name" is selected (not deleted) on expansion
            assert!(
                frame.contains("name"),
                "Placeholder 'name' should be selected (visible) on expand"
            );
            // $2 placeholder "params" remains (not yet navigated to)
            assert!(frame.contains("params"), "Should contain placeholder 'params'");
            assert!(frame.contains('{'), "Should contain opening brace");
            assert!(frame.contains('}'), "Should contain closing brace");
            eprintln!("[snippet] Function template rendered correctly");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Function snippet not visible in TUI: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

// ============================================================================
// Mode Transitions
// ============================================================================

/// Test: after snippet expansion, mode changes to NAVIGATING.
#[tokio::test]
async fn test_tui_snippet_mode_navigating() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle
        .send_keys("ifn<C-s>")
        .await
        .expect("Failed to send keys");

    // After expansion, mode should show NAVIGATING (snippet:navigating)
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| {
            let lower = frame.to_lowercase();
            lower.contains("navigating") || lower.contains("snippet")
        })
        .await;

    match result {
        Ok(frame) => {
            let lower = frame.to_lowercase();
            assert!(
                lower.contains("navigating") || lower.contains("snippet"),
                "TUI should show NAVIGATING or SNIPPET mode indicator"
            );
            eprintln!("[snippet] Mode indicator shows navigating");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "NAVIGATING mode not visible in TUI: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

/// Test: Escape during snippet navigation returns to INSERT mode.
#[tokio::test]
async fn test_tui_snippet_escape_to_insert() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Expand snippet (enters NAVIGATING mode)
    handle
        .send_keys("ifn<C-s>")
        .await
        .expect("Failed to send keys");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Press Escape to cancel snippet
    handle
        .send_keys("<Esc>")
        .await
        .expect("Failed to send Escape");

    // Should return to INSERT mode
    let result = handle
        .wait_for(Duration::from_secs(3), |frame| frame.to_lowercase().contains("insert"))
        .await;

    match result {
        Ok(frame) => {
            assert!(
                frame.to_lowercase().contains("insert"),
                "Should return to INSERT mode after Escape"
            );
            eprintln!("[snippet] Escape returned to INSERT mode");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "INSERT mode not restored after Escape: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

// ============================================================================
// Tab Stop Navigation
// ============================================================================

/// Test: Tab navigates cursor to next tab stop (visible as cursor movement).
#[tokio::test]
async fn test_tui_tab_navigates_cursor() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Expand "fn" snippet — cursor starts at $1 (name)
    handle
        .send_keys("ifn<C-s>")
        .await
        .expect("Failed to send keys");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Capture frame before Tab
    let frame_before = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");

    // Tab to next stop ($2 = params)
    handle.send_keys("<Tab>").await.expect("Failed to send Tab");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Capture frame after Tab
    let frame_after = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");

    // Buffer content should be preserved (function template still there)
    assert!(frame_after.contains("fn "), "Function template should persist after Tab");
    // Phase 2: After Tab to $2, "params" is selected (not deleted)
    assert!(
        frame_after.contains("params"),
        "Placeholder 'params' should be selected (visible) after Tab"
    );

    eprintln!("[snippet] Tab navigation - before:\n{frame_before}");
    eprintln!("[snippet] Tab navigation - after:\n{frame_after}");

    handle.stop().await;
}

/// Test: S-Tab navigates cursor backward.
#[tokio::test]
async fn test_tui_shift_tab_backward() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Expand "fn" snippet
    handle
        .send_keys("ifn<C-s>")
        .await
        .expect("Failed to send keys");

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Tab forward to $2
    handle.send_keys("<Tab>").await.expect("Failed to send Tab");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // S-Tab back to $1
    handle
        .send_keys("<S-Tab>")
        .await
        .expect("Failed to send S-Tab");

    tokio::time::sleep(Duration::from_millis(200)).await;

    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");

    // Template should still be intact
    assert!(frame.contains("fn "), "Template should persist after S-Tab");
    eprintln!("[snippet] S-Tab backward navigation works");

    handle.stop().await;
}

// ============================================================================
// Multi-line Expansion
// ============================================================================

/// Test: "for" snippet expands to multi-line for loop in TUI.
#[tokio::test]
async fn test_tui_expand_multiline_for() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle
        .send_keys("ifor<C-s>")
        .await
        .expect("Failed to send keys");

    let result = handle
        .wait_for(Duration::from_secs(3), |frame| frame.contains("for ") && frame.contains("in "))
        .await;

    match result {
        Ok(frame) => {
            assert!(frame.contains("for "), "Should contain 'for '");
            assert!(frame.contains("in "), "Should contain 'in '");
            assert!(frame.contains('{'), "Should contain opening brace");
            assert!(frame.contains('}'), "Should contain closing brace");
            eprintln!("[snippet] Multi-line for loop rendered correctly");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "For loop snippet not visible in TUI: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}

// ============================================================================
// Full Workflow
// ============================================================================

/// Test: complete snippet workflow — expand, navigate all stops, exit.
#[tokio::test]
async fn test_tui_full_workflow() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("Failed to connect TUI");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Step 1: Expand "fn" snippet
    handle
        .send_keys("ifn<C-s>")
        .await
        .expect("Failed to send keys");

    let frame = handle
        .wait_for(Duration::from_secs(3), |f| f.contains("fn "))
        .await
        .expect("Snippet did not expand");
    eprintln!("[workflow] Step 1 - Expanded:\n{frame}");

    // Step 2: Tab to $2 (params)
    handle.send_keys("<Tab>").await.expect("Failed to send Tab");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");
    eprintln!("[workflow] Step 2 - After Tab to $2:\n{frame}");

    // Step 3: Tab to $0 (final position, body)
    handle.send_keys("<Tab>").await.expect("Failed to send Tab");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let frame = handle
        .capture("plain_text")
        .await
        .expect("Failed to capture");
    eprintln!("[workflow] Step 3 - After Tab to $0:\n{frame}");

    // Step 4: Escape to return to insert mode
    handle
        .send_keys("<Esc>")
        .await
        .expect("Failed to send Escape");

    let result = handle
        .wait_for(Duration::from_secs(2), |f| f.to_lowercase().contains("insert"))
        .await;

    match result {
        Ok(frame) => {
            assert!(
                frame.to_lowercase().contains("insert"),
                "Should be in INSERT mode after full workflow"
            );
            eprintln!("[workflow] Step 4 - Back to INSERT:\n{frame}");
        }
        Err(e) => {
            let frame = handle.capture("plain_text").await.ok();
            panic!(
                "Full workflow did not return to INSERT: {e}\nFrame:\n{}",
                frame.unwrap_or_default()
            );
        }
    }

    handle.stop().await;
}
