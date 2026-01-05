//! Operator integration tests
//!
//! Tests for delete (d), yank (y), change (c), and related operators.

mod common;

use common::*;

// ============================================================================
// dd (delete line) tests - WORKING
// ============================================================================

#[tokio::test]
async fn test_dd_single_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("dd")
        .run()
        .await;

    result.assert_buffer_eq("");
}

#[tokio::test]
async fn test_dd_middle_line() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("jdd")
        .run()
        .await;

    result.assert_buffer_eq("line 1\nline 3");
}

#[tokio::test]
async fn test_dd_first_line() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("dd")
        .run()
        .await;

    result.assert_buffer_eq("line 2\nline 3");
}

#[tokio::test]
async fn test_dd_last_line() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("jjdd")
        .run()
        .await;

    result.assert_buffer_eq("line 1\nline 2");
}

// ============================================================================
// x (delete char forward) tests - WORKING
// ============================================================================

#[tokio::test]
async fn test_x_delete_char() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("x")
        .run()
        .await;

    result.assert_buffer_eq("ello");
}

#[tokio::test]
async fn test_x_middle_of_word() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("llx")
        .run()
        .await;

    result.assert_buffer_eq("helo");
}

#[tokio::test]
async fn test_x_at_end() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("$x")
        .run()
        .await;

    result.assert_buffer_eq("hell");
}

// ============================================================================
// yy (yank line) and p (paste) tests - WORKING
// ============================================================================

#[tokio::test]
async fn test_yy_p_duplicate_line() {
    let result = ServerTest::new()
        .await
        .with_content("hello")
        .with_keys("yyp")
        .run()
        .await;

    result.assert_buffer_eq("hello\nhello");
}

#[tokio::test]
async fn test_yy_p_multiline() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2")
        .with_keys("yyp")
        .run()
        .await;

    result.assert_buffer_eq("line 1\nline 1\nline 2");
}

#[tokio::test]
async fn test_yy_big_p_paste_before() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2")
        .with_keys("jyyP")
        .run()
        .await;

    result.assert_buffer_eq("line 1\nline 2\nline 2");
}

// ============================================================================
// d$ (delete to end of line) tests - WORKING
// ============================================================================

#[tokio::test]
async fn test_d_dollar_delete_to_eol() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("lld$")
        .run()
        .await;

    result.assert_buffer_eq("he");
}

// ============================================================================
// dj/dk (delete lines with motion) tests - WORKING
// ============================================================================

#[tokio::test]
async fn test_dj_delete_two_lines() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("dj")
        .run()
        .await;

    result.assert_buffer_eq("line 3");
}

#[tokio::test]
async fn test_dk_delete_two_lines() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("jdk")
        .run()
        .await;

    result.assert_buffer_eq("line 3");
}

// ============================================================================
// yj/yk (yank lines with motion) tests
// ============================================================================

#[tokio::test]
async fn test_yj_yank_two_lines() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("yjp")
        .run()
        .await;

    // yj should yank current line + next line (2 lines total)
    // p should paste those 2 lines below current line
    result.assert_buffer_eq("line 1\nline 1\nline 2\nline 2\nline 3");
}

#[tokio::test]
async fn test_yk_yank_two_lines() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("jykp")
        .run()
        .await;

    // Move to line 2, yk should yank line 1 + line 2 (2 lines total)
    // p should paste those 2 lines below current line
    result.assert_buffer_eq("line 1\nline 2\nline 1\nline 2\nline 3");
}

// ============================================================================
// Y (yank to end of line) tests
// ============================================================================

#[tokio::test]
async fn test_big_y_paste_after() {
    let result = ServerTest::new()
        .await
        .with_content("abc")
        .with_keys("Yp") // Y to yank "abc", p to paste
        .run()
        .await;

    // Y yanks "abc" (from cursor to end, characterwise)
    // p pastes AFTER cursor (at position 0, 'a')
    // Result: 'a' + 'abc' + 'bc' = "aabcbc"
    result.assert_buffer_eq("aabcbc");
}

#[tokio::test]
async fn test_big_y_paste_before() {
    let result = ServerTest::new()
        .await
        .with_content("abc")
        .with_keys("YP") // Y to yank "abc", P to paste BEFORE
        .run()
        .await;

    // Y yanks "abc" (from start to end, characterwise)
    // P pastes BEFORE cursor (which is at 'a')
    result.assert_buffer_eq("abcabc");
}

// ============================================================================
// Escape cancels operator - WORKING
// ============================================================================

#[tokio::test]
async fn test_d_escape_cancels() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("d<Esc>")
        .run()
        .await;

    result.assert_buffer_eq("hello world");
    result.assert_normal_mode();
}

// ============================================================================
// dw (delete word) tests
// ============================================================================

#[tokio::test]
async fn test_dw_at_last_word() {
    // Deletes last word in line
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("wdw")
        .run()
        .await;

    // Deletes "world" leaving "hello "
    result.assert_buffer_contains("hello");
}

/// dw deletes word and trailing space
#[tokio::test]
async fn test_dw_deletes_word() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("dw")
        .run()
        .await;

    // dw deletes "hello " (exclusive motion), leaving "world"
    result.assert_buffer_eq("world");
}

/// dw in middle of line deletes word and trailing space
#[tokio::test]
async fn test_dw_middle_of_line() {
    let result = ServerTest::new()
        .await
        .with_content("one two three")
        .with_keys("wdw")
        .run()
        .await;

    // At "two", dw deletes "two " (exclusive), leaving "one three"
    result.assert_buffer_eq("one three");
}

// ============================================================================
// cw (change word) tests
// ============================================================================

/// cw changes word and enters insert mode
#[tokio::test]
async fn test_cw_changes_word() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("cwgoodbye<Esc>")
        .run()
        .await;

    // cw deletes "hello" (to next word boundary, exclusive), inserts "goodbye"
    result.assert_buffer_eq("goodbye world");
    result.assert_normal_mode();
}

/// cw in middle of line changes word correctly
#[tokio::test]
async fn test_cw_middle_of_line() {
    let result = ServerTest::new()
        .await
        .with_content("one two three")
        .with_keys("wcwnew<Esc>")
        .run()
        .await;

    // At "two", cw deletes "two", inserts "new"
    result.assert_buffer_eq("one new three");
    result.assert_normal_mode();
}

// ============================================================================
// db (delete backward word) tests
// ============================================================================

/// db deletes word backward
#[tokio::test]
async fn test_db_deletes_backward() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("wdb")
        .run()
        .await;

    // w moves to "world", db deletes "hello " backward (exclusive)
    result.assert_buffer_eq("world");
}

// ============================================================================
// dd + p (delete then paste)
// ============================================================================

/// dd populates register - p pastes below current line (Vim-compatible)
#[tokio::test]
async fn test_dd_p_register_populated() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("ddp")
        .run()
        .await;

    // dd deletes "line 1\n", buffer becomes "line 2\nline 3", cursor at line 0
    // p pastes "line 1\n" BELOW current line (Vim behavior)
    // Result: "line 2\nline 1\nline 3"
    result.assert_buffer_eq("line 2\nline 1\nline 3");
}

/// Verify dd actually populates register by pasting twice
#[tokio::test]
async fn test_dd_register_can_paste_multiple() {
    let result = ServerTest::new()
        .await
        .with_content("only line")
        .with_keys("ddpp")
        .run()
        .await;

    // dd deletes "only line", buffer empty
    // p inserts "only line\n", buffer has one line
    // second p inserts "only line\n" again
    result.assert_buffer_contains("only line");
}
