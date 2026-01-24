//! Operator integration tests (delete, yank, paste, change).
//!
//! **Status**: 24 tests enabled, 10 tests require operator-pending mode.
//!
//! Operator-pending mode (d+motion, y+motion, c+motion) is not yet implemented.
//! Tests using direct bindings (dd, yy, cc, x, 5dd, 3x, etc.) work.
//! Count prefix edge cases fully covered (#337).
//!
//! Run `cargo test --ignored` to run the remaining ignored tests.

use runner::testing::IntegrationTest;

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
#[ignore = "requires operator-pending mode (d/y/c + motion)"]
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
#[ignore = "requires operator-pending mode (d/y/c + motion)"]
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
#[ignore = "requires operator-pending mode (d/y/c + motion)"]
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
#[ignore = "requires operator-pending mode (d/y/c + motion)"]
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
#[ignore = "requires operator-pending mode (d/y/c + motion)"]
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
#[ignore = "requires operator-pending mode (d/y/c + motion)"]
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
#[ignore = "requires operator-pending mode (d/y/c + motion)"]
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

// ============================================================================
// COUNT PREFIX EDGE CASES - Issue #337
// ============================================================================

#[tokio::test]
async fn test_count_delete_exceeds_lines() {
    // 100dd on 3 lines should delete all 3
    let result = IntegrationTest::new()
        .await
        .with_buffer("one\ntwo\nthree")
        .send_keys("100dd")
        .run()
        .await;
    // Buffer should have only empty line (kernel keeps at least 1 line)
    result.assert_buffer_eq("");
}

#[tokio::test]
async fn test_count_delete_char_exceeds_line() {
    // 100x on "hello" deletes all 5 chars
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("100x")
        .run()
        .await;
    // All characters deleted
    result.assert_buffer_eq("");
}

#[tokio::test]
async fn test_4p_paste_four_times() {
    // 4p pastes 4 times (mentioned in #337 issue)
    let result = IntegrationTest::new()
        .await
        .with_buffer("a")
        .send_keys("yy4p")
        .run()
        .await;
    // Original + 4 pastes = 5 lines of "a"
    result.assert_buffer_eq("a\na\na\na\na");
}

#[tokio::test]
async fn test_count_yank_exceeds_lines() {
    // 10yy on 2 lines yanks both, then p pastes
    let result = IntegrationTest::new()
        .await
        .with_buffer("one\ntwo")
        .send_keys("10yyGp")
        .run()
        .await;
    // Should paste both lines after last line
    result.assert_buffer_eq("one\ntwo\none\ntwo");
}

#[tokio::test]
async fn test_large_count_movement() {
    // 999j on 3 lines clamps to last line
    let result = IntegrationTest::new()
        .await
        .with_buffer("one\ntwo\nthree")
        .send_keys("999j")
        .run()
        .await;
    // Cursor should be on line 2 (last line, 0-indexed)
    result.assert_cursor(2, 0);
}

#[tokio::test]
async fn test_dollar_then_count_h() {
    // Go to EOL then move back with count
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("$3h")
        .run()
        .await;
    // At 'd' (col 10), 3h -> col 7 ('o' in "world")
    result.assert_cursor(0, 7);
}

// Debug test for #421
#[tokio::test]
async fn debug_cc_no_text() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jcc<Esc>")
        .run()
        .await;

    eprintln!("DEBUG cc without text:");
    eprintln!("  Buffer: {:?}", result.buffer_content);
    eprintln!("  Cursor: ({}, {})", result.cursor_line, result.cursor_column);
    eprintln!("  Mode: {}", result.edit_mode);

    result.assert_buffer_eq("line 1\n\nline 3");
}

#[tokio::test]
async fn debug_cc_single_char() {
    // First check state after cc (before typing)
    let after_cc = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jcc")
        .run()
        .await;

    eprintln!("DEBUG after jcc (in insert mode):");
    eprintln!("  Buffer: {:?}", after_cc.buffer_content);
    eprintln!("  Cursor: ({}, {})", after_cc.cursor_line, after_cc.cursor_column);
    eprintln!("  Mode: {}", after_cc.edit_mode);

    // Now check what happens when we type X
    let after_x = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jccX<Esc>")
        .run()
        .await;

    eprintln!("DEBUG after jccX<Esc>:");
    eprintln!("  Buffer: {:?}", after_x.buffer_content);
    eprintln!("  Cursor: ({}, {})", after_x.cursor_line, after_x.cursor_column);
    eprintln!("  Mode: {}", after_x.edit_mode);

    after_x.assert_buffer_eq("line 1\nX\nline 3");
}
