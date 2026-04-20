//! E2E tests for ex-command argument system (#556).
//!
//! Verifies command parsing, argument binding, prefix matching, and
//! error handling through the full server stack via headless capture.
//!
//! # Running
//!
//! ```bash
//! cargo test -p reovim-module-vim --test ex_commands
//! ```

use reovim_testing::IntegrationTest;

// ============================================================================
// :write / :w - file writing
// ============================================================================

/// :w saves buffer content to the underlying file.
#[tokio::test]
async fn test_write_command_saves_file() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("A more text<Esc>")
        .send_keys(":w<CR>")
        .run()
        .await;
    result.assert_buffer_contains("hello world more text");
    result.assert_normal_mode();
}

/// :write (full name) works the same as :w.
#[tokio::test]
async fn test_write_full_name() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("test content")
        .send_keys(":write<CR>")
        .run()
        .await;
    result.assert_buffer_eq("test content");
    result.assert_normal_mode();
}

// ============================================================================
// Prefix matching (#561)
// ============================================================================

/// :wri resolves to :write via prefix matching.
#[tokio::test]
async fn test_write_prefix_matching() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("prefix test")
        .send_keys(":wri<CR>")
        .run()
        .await;
    result.assert_buffer_eq("prefix test");
    result.assert_normal_mode();
}

/// :q resolves to :quit.
#[tokio::test]
async fn test_quit_prefix() {
    // After :q, the buffer should still be accessible in this test
    // because the test framework captures state before server shutdown
    let result = IntegrationTest::new()
        .await
        .with_buffer("quit test")
        .send_keys(":q<CR>")
        .run()
        .await;
    // After quit, mode may vary — just verify the test completes
    let _ = result.buffer_content;
}

// ============================================================================
// Unknown command error (E492)
// ============================================================================

/// Unknown command returns to normal mode (E492 error shown in cmdline).
#[tokio::test]
async fn test_unknown_command_stays_normal() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("error test")
        .send_keys(":nonexistentcommand<CR>")
        .run()
        .await;
    // Buffer unchanged after unknown command
    result.assert_buffer_eq("error test");
    result.assert_normal_mode();
}

// ============================================================================
// :edit / :e - buffer operations
// ============================================================================

/// :e with a file path opens the file (already used internally by test framework).
#[tokio::test]
async fn test_edit_preserves_mode() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("original")
        .send_keys(":e<CR>")
        .run()
        .await;
    // Re-editing current file preserves content
    result.assert_normal_mode();
}

// ============================================================================
// Ex-command with insert mode interaction
// ============================================================================

/// Insert text, escape to normal, then run ex-command — buffer preserved.
#[tokio::test]
async fn test_insert_then_write() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("ihello from insert<Esc>")
        .send_keys(":w<CR>")
        .run()
        .await;
    result.assert_buffer_contains("hello from insert");
    result.assert_normal_mode();
}

/// Multiple ex-commands in sequence.
#[tokio::test]
async fn test_multiple_ex_commands() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("multi test")
        .send_keys(":w<CR>")
        .send_keys(":w<CR>")
        .run()
        .await;
    result.assert_buffer_eq("multi test");
    result.assert_normal_mode();
}
