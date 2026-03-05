//! E2E tests for range-finder module.
//!
//! Verifies that range-finder keybindings are registered and don't crash
//! the server when pressed. Command `execute()` bodies are stubs in Phase 4,
//! so these tests verify module loading and key dispatch only.

use reovim_testing::{IntegrationTest, StepTest};

// ============================================================================
// Module Loading & Key Registration
// ============================================================================

/// Pressing `s` (jump search) should not crash and should stay in normal mode
/// (stub command returns Success without changing mode).
#[tokio::test]
async fn test_s_key_does_not_crash() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("s")
        .run()
        .await;

    // Buffer unchanged (stub is no-op)
    result.assert_buffer_eq("hello world");
}

/// Pressing `za` (fold toggle) should not crash.
#[tokio::test]
async fn test_za_fold_toggle_does_not_crash() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("fn main() {\n    println!(\"hello\");\n}")
        .send_keys("za")
        .run()
        .await;

    result.assert_buffer_eq("fn main() {\n    println!(\"hello\");\n}");
}

/// Pressing `zo` (fold open) should not crash.
#[tokio::test]
async fn test_zo_fold_open_does_not_crash() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("fn main() {\n    println!(\"hello\");\n}")
        .send_keys("zo")
        .run()
        .await;

    result.assert_buffer_eq("fn main() {\n    println!(\"hello\");\n}");
}

/// Pressing `zc` (fold close) should not crash.
#[tokio::test]
async fn test_zc_fold_close_does_not_crash() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("fn main() {\n    println!(\"hello\");\n}")
        .send_keys("zc")
        .run()
        .await;

    result.assert_buffer_eq("fn main() {\n    println!(\"hello\");\n}");
}

/// Pressing `zR` (open all folds) should not crash.
#[tokio::test]
async fn test_zr_fold_open_all_does_not_crash() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("fn main() {\n    println!(\"hello\");\n}")
        .send_keys("zR")
        .run()
        .await;

    result.assert_buffer_eq("fn main() {\n    println!(\"hello\");\n}");
}

/// Pressing `zM` (close all folds) should not crash.
#[tokio::test]
async fn test_zm_fold_close_all_does_not_crash() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("fn main() {\n    println!(\"hello\");\n}")
        .send_keys("zM")
        .run()
        .await;

    result.assert_buffer_eq("fn main() {\n    println!(\"hello\");\n}");
}

// ============================================================================
// Step Tests - Per-Key State Tracking
// ============================================================================

/// Verify `s` key dispatch step-by-step.
#[tokio::test]
async fn test_s_key_step_by_step() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello world")
        .step("s")
        .expect_buffer("hello world")
        .run()
        .await;

    trace.assert_ok();
}

/// Verify fold keys `za` dispatch step-by-step.
#[tokio::test]
async fn test_za_step_by_step() {
    let trace = StepTest::new()
        .await
        .with_buffer("fn main() {\n    hello\n}")
        .step("z")
        .expect_buffer("fn main() {\n    hello\n}")
        .step("a")
        .expect_buffer("fn main() {\n    hello\n}")
        .run()
        .await;

    trace.assert_ok();
}
