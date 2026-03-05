//! E2E tests for snippet expansion (#136).
//!
//! Tests verify the full snippet workflow through the server/client gRPC stack:
//! type trigger prefix -> `<C-s>` expand -> Tab/S-Tab navigate -> Esc cancel.
//!
//! Snippet fixtures are created in a temp directory per-test so tests are
//! self-contained and work in CI without pre-installed snippet files.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-module-snippet --test expansion
//! ```

use reovim_testing::{IntegrationTest, StepTest};

/// Snippet fixture content matching the test expectations.
const GLOBAL_SNIPPETS: &str = r#"{
  "function": {
    "prefix": "fn",
    "body": "fn ${1:name}(${2:params}) {\n\t$0\n}",
    "description": "Function definition"
  },
  "for_loop": {
    "prefix": "for",
    "body": "for ${1:i} in ${2:iter} {\n\t$0\n}",
    "description": "For loop"
  },
  "test_simple": {
    "prefix": "tst",
    "body": "TEST_EXPANDED",
    "description": "Simple test snippet (no tab stops)"
  }
}"#;

/// Create a temp directory with snippet fixtures and return the `XDG_DATA_HOME` value.
fn create_snippet_fixtures() -> (tempfile::TempDir, String) {
    let tmp = tempfile::tempdir().expect("Failed to create temp dir");
    let snippet_dir = tmp.path().join("reovim").join("modules").join("snippets");
    std::fs::create_dir_all(&snippet_dir).expect("Failed to create snippet dir");
    std::fs::write(snippet_dir.join("global.json"), GLOBAL_SNIPPETS)
        .expect("Failed to write global.json");
    let xdg_path = tmp.path().to_str().expect("Non-UTF8 temp path").to_string();
    (tmp, xdg_path)
}

// ============================================================================
// Basic Expansion
// ============================================================================

/// Test: type "tst" then `<C-s>` expands a simple snippet (no tab stops).
#[tokio::test]
async fn test_expand_simple_no_tabstops() {
    let (_tmp, xdg) = create_snippet_fixtures();
    let result = IntegrationTest::with_env(&[("XDG_DATA_HOME", &xdg)])
        .await
        .with_buffer("")
        .send_keys("itst<C-s>")
        .with_delay(100)
        .run()
        .await;

    // "tst" should be replaced with "TEST_EXPANDED"
    result.assert_buffer_contains("TEST_EXPANDED");
}

/// Test: type unknown prefix then `<C-s>` does nothing.
#[tokio::test]
async fn test_expand_no_match() {
    let (_tmp, xdg) = create_snippet_fixtures();
    let result = IntegrationTest::with_env(&[("XDG_DATA_HOME", &xdg)])
        .await
        .with_buffer("")
        .send_keys("ixyz<C-s>")
        .with_delay(100)
        .run()
        .await;

    // No expansion — buffer should just have "xyz"
    result.assert_buffer_eq("xyz");
}

/// Test: expand "fn" snippet produces function template.
#[tokio::test]
async fn test_expand_function_snippet() {
    let (_tmp, xdg) = create_snippet_fixtures();
    let result = IntegrationTest::with_env(&[("XDG_DATA_HOME", &xdg)])
        .await
        .with_buffer("")
        .send_keys("ifn<C-s>")
        .with_delay(100)
        .run()
        .await;

    // Should contain the expanded function body
    result.assert_buffer_contains("fn ");
    result.assert_buffer_contains("{");
    result.assert_buffer_contains("}");
}

// ============================================================================
// Tab Stop Navigation
// ============================================================================

/// Test: expand "fn" snippet then Tab navigates through tab stops.
#[tokio::test]
async fn test_tab_navigation() {
    let (_tmp, xdg) = create_snippet_fixtures();
    let trace = StepTest::with_env(&[("XDG_DATA_HOME", &xdg)])
        .await
        .with_buffer("")
        .step("ifn<C-s>")
        .expect_buffer_contains("fn ")
        .step("<Tab>") // Jump from $1 to $2
        .expect_buffer_contains("fn ")
        .step("<Tab>") // Jump from $2 to $0 (final position)
        .expect_buffer_contains("fn ")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: expand "fn" snippet then Esc cancels snippet mode.
#[tokio::test]
async fn test_escape_cancels_snippet() {
    let (_tmp, xdg) = create_snippet_fixtures();
    let trace = StepTest::with_env(&[("XDG_DATA_HOME", &xdg)])
        .await
        .with_buffer("")
        .step("ifn<C-s>")
        .expect_buffer_contains("fn ")
        .step("<Esc>")
        .expect_mode_contains("INSERT")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

/// Test: S-Tab navigates backward.
#[tokio::test]
async fn test_shift_tab_backward() {
    let (_tmp, xdg) = create_snippet_fixtures();
    let trace = StepTest::with_env(&[("XDG_DATA_HOME", &xdg)])
        .await
        .with_buffer("")
        .step("ifn<C-s>")
        .expect_buffer_contains("fn ")
        .step("<Tab>") // $1 → $2
        .expect_buffer_contains("fn ")
        .step("<S-Tab>") // $2 → $1 (back to first)
        .expect_buffer_contains("fn ")
        .run()
        .await;

    trace.print_trace();
    trace.assert_ok();
}

// ============================================================================
// Multi-line Expansion
// ============================================================================

/// Test: "for" snippet expands to multi-line for loop.
#[tokio::test]
async fn test_expand_multiline_for_loop() {
    let (_tmp, xdg) = create_snippet_fixtures();
    let result = IntegrationTest::with_env(&[("XDG_DATA_HOME", &xdg)])
        .await
        .with_buffer("")
        .send_keys("ifor<C-s>")
        .with_delay(100)
        .run()
        .await;

    result.assert_buffer_contains("for ");
    result.assert_buffer_contains("in ");
    result.assert_buffer_contains("{");
    result.assert_buffer_contains("}");
}
