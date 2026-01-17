//! Operator integration tests (delete, yank, paste, change).
//!
//! **Status**: 19 tests enabled, 9 tests require additional features
//! ($ motion, operator-motion with j/w/b motions, paste before P).
//!
//! Run `cargo test --ignored` to run the remaining ignored tests.

mod common;
use common::IntegrationTest;

// ============================================================================
// DELETE OPERATORS (dd, x, d{motion})
// ============================================================================

#[tokio::test]
async fn test_dd_single_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("dd")
        .run()
        .await;
    result.assert_buffer_eq("");
}

#[tokio::test]
async fn test_dd_middle_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jdd")
        .run()
        .await;
    result.assert_buffer_eq("line 1\nline 3");
}

#[tokio::test]
async fn test_dd_first_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("dd")
        .run()
        .await;
    result.assert_buffer_eq("line 2\nline 3");
}

#[tokio::test]
async fn test_dd_last_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jjdd")
        .run()
        .await;
    result.assert_buffer_eq("line 1\nline 2");
}

#[tokio::test]
async fn test_5dd_count_delete() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("1\n2\n3\n4\n5\n6")
        .send_keys("5dd")
        .run()
        .await;
    result.assert_buffer_eq("6");
}

#[tokio::test]
async fn test_x_delete_char() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("x")
        .run()
        .await;
    result.assert_buffer_eq("ello");
}

#[tokio::test]
async fn test_x_middle_of_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("llx")
        .run()
        .await;
    result.assert_buffer_eq("helo");
}

#[tokio::test]
async fn test_x_at_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("$x")
        .run()
        .await;
    result.assert_buffer_eq("hell");
}

#[tokio::test]
async fn test_3x_count_delete() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("3x")
        .run()
        .await;
    result.assert_buffer_eq("lo");
}

#[tokio::test]
#[ignore = "requires w motion to return range in operator-pending mode"]
async fn test_dw_delete_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("dw")
        .run()
        .await;
    result.assert_buffer_eq("world");
}

#[tokio::test]
#[ignore = "requires $ motion (#340)"]
async fn test_d_dollar_delete_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("lld$")
        .run()
        .await;
    result.assert_buffer_eq("he");
}

#[tokio::test]
#[ignore = "requires j motion to return range in operator-pending mode"]
async fn test_dj_delete_two_lines() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("dj")
        .run()
        .await;
    result.assert_buffer_eq("line 3");
}

#[tokio::test]
#[ignore = "requires b motion to return range in operator-pending mode"]
async fn test_db_delete_backward_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("wdb")
        .run()
        .await;
    result.assert_buffer_eq("world");
}

// ============================================================================
// YANK OPERATORS (yy, Y, y{motion})
// ============================================================================

#[tokio::test]
async fn test_yy_p_duplicate_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("yyp")
        .run()
        .await;
    result.assert_buffer_eq("hello\nhello");
}

#[tokio::test]
async fn test_yy_p_multiline() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2")
        .send_keys("yyp")
        .run()
        .await;
    result.assert_buffer_eq("line 1\nline 1\nline 2");
}

#[tokio::test]
#[ignore = "requires j motion to return range in operator-pending mode"]
async fn test_yj_yank_two_lines() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("yjp")
        .run()
        .await;
    result.assert_buffer_eq("line 1\nline 1\nline 2\nline 2\nline 3");
}

#[tokio::test]
async fn test_yw_yank_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("ywwP")
        .run()
        .await;
    result.assert_buffer_contains("hello ");
}

#[tokio::test]
async fn test_2yy_yank_count() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("2yyp")
        .run()
        .await;
    result.assert_buffer_eq("line 1\nline 1\nline 2\nline 2\nline 3");
}

// ============================================================================
// PASTE OPERATORS (p, P)
// ============================================================================

#[tokio::test]
async fn test_dd_p_paste_after() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("ddp")
        .run()
        .await;
    result.assert_buffer_eq("line 2\nline 1\nline 3");
}

#[tokio::test]
#[ignore = "requires P (paste before) fix"]
async fn test_dd_upper_p_paste_before() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jddP")
        .run()
        .await;
    result.assert_buffer_eq("line 2\nline 1\nline 3");
}

#[tokio::test]
async fn test_yw_p_paste_char() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("yw$p")
        .run()
        .await;
    result.assert_buffer_contains("hello ");
}

#[tokio::test]
async fn test_2p_paste_multiple() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("yy2p")
        .run()
        .await;
    result.assert_buffer_eq("hello\nhello\nhello");
}

#[tokio::test]
async fn test_paste_preserves_register() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("yyppp")
        .run()
        .await;
    result.assert_buffer_eq("hello\nhello\nhello\nhello");
}

// ============================================================================
// CHANGE OPERATORS (c, cw, c$, cc)
// ============================================================================

#[tokio::test]
#[ignore = "requires w motion to return range in operator-pending mode"]
async fn test_cw_change_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("cwgoodbye<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("goodbye world");
    result.assert_normal_mode();
}

#[tokio::test]
#[ignore = "requires $ motion (#340)"]
async fn test_c_dollar_change_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("llc$XXX<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("heXXX");
}

#[tokio::test]
async fn test_cc_change_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jccnew line<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("line 1\nnew line\nline 3");
}

#[tokio::test]
#[ignore = "requires w motion + count support in operator-pending mode"]
async fn test_2cw_change_count() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("one two three")
        .send_keys("2cwXXX<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("XXXthree");
}

// ============================================================================
// ESCAPE CANCELS OPERATOR
// ============================================================================

#[tokio::test]
async fn test_d_escape_cancels() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("d<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("hello world");
    result.assert_normal_mode();
}
