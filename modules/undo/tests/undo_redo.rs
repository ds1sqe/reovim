//! Undo/redo tests (u, Ctrl-R).
//!
//! **Status**: 5 tests enabled, 3 tests require full undo implementation (#385).
//! - `test_undo_delete_line` - single dd undo not working
//! - `test_multiple_undo` - multiple undo steps not working
//! - `test_undo_insert` - insert mode undo not working
//!
//! Run `cargo test --ignored` to run the remaining ignored tests.

use runner::testing::{IntegrationTest, StepTest};

#[tokio::test]
async fn test_undo_delete_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("ddu")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

#[tokio::test]
async fn test_redo_after_undo() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("ddu<C-r>")
        .run()
        .await;
    result.assert_buffer_eq("");
}

#[tokio::test]
async fn test_multiple_undo() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("dddduu")
        .run()
        .await;
    result.assert_buffer_eq("line 1\nline 2\nline 3");
}

#[tokio::test]
async fn test_undo_insert() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("ihello<Esc>u")
        .run()
        .await;
    result.assert_buffer_eq("");
}

#[tokio::test]
async fn test_multiple_redo() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("dddduuu<C-r><C-r>")
        .run()
        .await;
    result.assert_buffer_eq("line 3");
}

#[tokio::test]
async fn test_undo_change_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("cwXXX<Esc>u")
        .run()
        .await;
    result.assert_buffer_eq("hello world");
}

#[tokio::test]
async fn test_undo_nothing_to_undo() {
    // Undo on fresh buffer should not crash
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("u")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

#[tokio::test]
async fn test_redo_nothing_to_redo() {
    // Redo without prior undo should not crash
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("<C-r>")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

// ============================================================================
// DEBUG: Step-by-step tests to trace insert mode issue
// ============================================================================

/// Debug test for insert mode - trace each key step by step
#[tokio::test]
async fn debug_step_insert_escape() {
    let trace = StepTest::new()
        .await
        .with_buffer("")
        .step("i")
        .expect_mode_contains("INSERT")
        .step("h")
        .expect_buffer("h")
        .step("e")
        .expect_buffer("he")
        .step("l")
        .step("l")
        .step("o")
        .expect_buffer("hello")
        .step("<Esc>")
        .expect_mode_contains("NORMAL")
        .step("u")
        .expect_buffer("")
        .run()
        .await;

    trace.print_trace();

    let final_state = trace.final_state();
    eprintln!("\nFinal buffer: {:?}", final_state.buffer);
    eprintln!("Final mode: {} ({})", final_state.mode_display, final_state.edit_mode);
}
