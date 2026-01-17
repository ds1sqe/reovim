//! Cursor movement tests (hjkl, 0$, gg/G, w/b/e).
//!
//! **Status**: All 17 tests enabled.
//! Count prefix support (3j, 5G) implemented in RPC input handler.

mod common;
use common::IntegrationTest;

// ============================================================================
// BASIC MOVEMENTS (hjkl)
// ============================================================================

#[tokio::test]
async fn test_j_moves_down() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("j")
        .run()
        .await;
    result.assert_cursor(1, 0);
}

#[tokio::test]
async fn test_k_moves_up() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jjk")
        .run()
        .await;
    result.assert_cursor(1, 0);
}

#[tokio::test]
async fn test_l_moves_right() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("ll")
        .run()
        .await;
    result.assert_cursor(0, 2);
}

#[tokio::test]
async fn test_h_moves_left() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("lllh")
        .run()
        .await;
    result.assert_cursor(0, 2);
}

#[tokio::test]
async fn test_3j_count_movement() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("1\n2\n3\n4\n5")
        .send_keys("3j")
        .run()
        .await;
    result.assert_cursor(3, 0);
}

// ============================================================================
// LINE MOVEMENTS (0, $, ^)
// ============================================================================

#[tokio::test]
async fn test_dollar_moves_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("$")
        .run()
        .await;
    result.assert_cursor(0, 10); // 'd' is at column 10
}

#[tokio::test]
async fn test_0_moves_to_bol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("$0")
        .run()
        .await;
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_caret_moves_to_first_nonblank() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("   hello")
        .send_keys("^")
        .run()
        .await;
    result.assert_cursor(0, 3);
}

// ============================================================================
// FILE MOVEMENTS (gg, G)
// ============================================================================

#[tokio::test]
async fn test_gg_moves_to_first_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("Ggg")
        .run()
        .await;
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_g_moves_to_last_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("G")
        .run()
        .await;
    result.assert_cursor(2, 0);
}

#[tokio::test]
async fn test_5g_moves_to_line_5() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("1\n2\n3\n4\n5\n6\n7")
        .send_keys("5G")
        .run()
        .await;
    result.assert_cursor(4, 0); // Line 5 is index 4
}

// ============================================================================
// WORD MOVEMENTS (w, b, e)
// ============================================================================

#[tokio::test]
async fn test_w_moves_to_next_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("w")
        .run()
        .await;
    result.assert_cursor(0, 6); // 'w' of "world"
}

#[tokio::test]
async fn test_b_moves_to_prev_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("wb")
        .run()
        .await;
    result.assert_cursor(0, 0); // back to 'h'
}

#[tokio::test]
async fn test_e_moves_to_end_of_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("e")
        .run()
        .await;
    result.assert_cursor(0, 4); // 'o' of "hello"
}

// ============================================================================
// BOUNDARY CONDITIONS
// ============================================================================

#[tokio::test]
async fn test_j_at_last_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2")
        .send_keys("jj") // Second j should do nothing
        .run()
        .await;
    result.assert_cursor(1, 0);
}

#[tokio::test]
async fn test_h_at_bol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("h") // Already at beginning
        .run()
        .await;
    result.assert_cursor(0, 0);
}

#[tokio::test]
async fn test_l_at_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hi")
        .send_keys("lll") // Can't go past last char
        .run()
        .await;
    result.assert_cursor(0, 1); // At 'i'
}
