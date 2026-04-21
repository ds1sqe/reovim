//! E2E tests for file explorer sidebar (#523).
//!
//! Tests verify the full explorer workflow through the server/client gRPC stack:
//! `<Space>e` toggle → j/k navigation → l/h expand/collapse → q close.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-module-explorer --test explorer
//! ```

use reovim_testing::StepTest;

// ============================================================================
// Toggle & Mode
// ============================================================================

/// Test: `<Space>e` toggles explorer and enters EXPLORER mode.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_toggle_explorer_enters_browse_mode() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: `<Space>e` twice toggles explorer off, returning to NORMAL.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_toggle_explorer_off_returns_normal() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("<Space>e")
        .expect_mode_contains("NORMAL")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: `q` closes explorer and returns to NORMAL mode.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_close_explorer_with_q() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("q")
        .expect_mode_contains("NORMAL")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: `<Esc>` closes explorer and returns to NORMAL mode.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_close_explorer_with_escape() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("<Esc>")
        .expect_mode_contains("NORMAL")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

// ============================================================================
// Navigation
// ============================================================================

/// Test: j/k navigation maintains EXPLORER mode.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_jk_navigation_stays_in_explorer() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("j")
        .expect_mode_contains("EXPLORER")
        .step("k")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: gg goes to first, G goes to last — mode stays EXPLORER.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_gg_and_big_g_navigation() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("G")
        .expect_mode_contains("EXPLORER")
        .step("gg")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

// ============================================================================
// Expand / Collapse
// ============================================================================

/// Test: l expands (or opens) and h collapses — mode stays EXPLORER.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_expand_collapse_with_l_h() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("l")
        .expect_mode_contains("EXPLORER")
        .step("h")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

// ============================================================================
// Input Mode
// ============================================================================

/// Test: `a` in explorer enters input mode (create file).
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_create_file_enters_input_mode() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("a")
        .expect_mode_contains("EXPLORER_INPUT")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: `<Esc>` in input mode returns to browse mode.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_escape_from_input_returns_to_browse() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("a")
        .expect_mode_contains("EXPLORER_INPUT")
        .step("<Esc>")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: `A` in explorer enters input mode (create directory).
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_create_dir_enters_input_mode() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("A")
        .expect_mode_contains("EXPLORER_INPUT")
        .step("<Esc>")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: `r` in explorer enters input mode (rename).
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_rename_enters_input_mode() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("r")
        .expect_mode_contains("EXPLORER_INPUT")
        .step("<Esc>")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: `d` in explorer enters input mode (confirm delete).
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_delete_enters_input_mode() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("d")
        .expect_mode_contains("EXPLORER_INPUT")
        .step("<Esc>")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: typing characters in input mode stays in `EXPLORER_INPUT`.
///
/// Before the `InputResolver` fix, chars were silently dropped
/// (returned `NotHandled`), which could cause mode fallback.
/// After the fix, chars are routed to `ExplorerState`'s `TextInputSink`.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_input_mode_typing_stays_in_input() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("a")
        .expect_mode_contains("EXPLORER_INPUT")
        .step("t")
        .expect_mode_contains("EXPLORER_INPUT")
        .step("e")
        .expect_mode_contains("EXPLORER_INPUT")
        .step("s")
        .expect_mode_contains("EXPLORER_INPUT")
        .step("t")
        .expect_mode_contains("EXPLORER_INPUT")
        .step("<Esc>")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

// ============================================================================
// Toggle Hidden & Refresh
// ============================================================================

/// Test: `H` toggles hidden files — stays in EXPLORER.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_toggle_hidden() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("H")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: `R` refreshes tree — stays in EXPLORER.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_refresh() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("R")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

// ============================================================================
// Open file with Enter
// ============================================================================

/// Test: `<CR>` on a file node opens it and returns to NORMAL mode.
///
/// Uses `G` (goto last) to navigate to the last visible node, which should
/// be a file (directories sort before files). Then `<CR>` opens it.
/// The buffer should change from the initial content to the file's content.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_open_file_with_enter() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(120)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .step("G")
        .expect_mode_contains("EXPLORER")
        .step("<CR>")
        .expect_mode_contains("NORMAL")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();

    // Verify the buffer changed from initial "hello" to the opened file
    let final_buf = &trace.final_state().buffer;
    eprintln!(
        "  Final buffer content (first 200 chars): {:?}",
        &final_buf[..final_buf.len().min(200)]
    );
    assert_ne!(final_buf, "hello", "Buffer should change after opening a file");
}

/// Test: `<CR>` on the root directory (index 0) toggles expand, stays in EXPLORER.
///
/// When cursor is at index 0 (root dir), Enter should toggle the directory,
/// not open a file. This verifies the user must navigate to a file node first.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_enter_on_root_dir_toggles() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello")
        .with_delay(120)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        // Cursor starts at index 0 = root directory
        .step("<CR>")
        .expect_mode_contains("EXPLORER")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

// ============================================================================
// Buffer stays intact
// ============================================================================

/// Test: explorer toggle/close doesn't modify buffer.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: StepTest send_keys raw-bytes, server requires 8-byte InputEvent — see #759"]
async fn test_explorer_does_not_modify_buffer() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello world")
        .with_delay(80)
        .step("<Space>e")
        .expect_mode_contains("EXPLORER")
        .expect_buffer("hello world")
        .step("jjkl")
        .expect_buffer("hello world")
        .step("q")
        .expect_mode_contains("NORMAL")
        .expect_buffer("hello world")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}
