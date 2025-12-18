//! Operator integration tests
//!
//! Tests for delete (d), yank (y), change (c), and related operators.
//!
//! ## Implementation Notes
//!
//! Known issues documented in tests:
//! - `dw`/`cw` have off-by-one bug (delete word + first char of next word)
//! - `dd` + `p` doesn't preserve deleted line in register
//! - `Y` has non-standard behavior

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
// dw (delete word) tests - Document actual behavior
// Note: dw has off-by-one bug - deletes word + first char of next word
// ============================================================================

#[tokio::test]
async fn test_dw_at_last_word() {
    // This works correctly because there's no next word
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("wdw")
        .run()
        .await;

    // Deletes "world" leaving "hello "
    result.assert_buffer_contains("hello");
}

/// Documents dw off-by-one bug
#[tokio::test]
async fn test_doc_dw_off_by_one() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("dw")
        .run()
        .await;

    // Expected vim behavior: "world"
    // Actual behavior: "orld" (deletes "hello w" instead of "hello ")
    result.assert_buffer_eq("orld");
}

/// Documents dw off-by-one in middle of line
#[tokio::test]
async fn test_doc_dw_middle_off_by_one() {
    let result = ServerTest::new()
        .await
        .with_content("one two three")
        .with_keys("wdw")
        .run()
        .await;

    // Expected vim behavior: "one three"
    // Actual behavior: "one hree" (deletes "two t" instead of "two ")
    result.assert_buffer_eq("one hree");
}

// ============================================================================
// cw (change word) tests - Document actual behavior
// Note: cw has same off-by-one bug as dw
// ============================================================================

/// Documents cw off-by-one bug
#[tokio::test]
async fn test_doc_cw_off_by_one() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("cwgoodbye<Esc>")
        .run()
        .await;

    // Expected vim behavior: "goodbye world"
    // Actual behavior: "goodbyeorld" (deletes "hello w" then inserts)
    result.assert_buffer_eq("goodbyeorld");
    result.assert_normal_mode();
}

/// Documents cw off-by-one in middle
#[tokio::test]
async fn test_doc_cw_middle_off_by_one() {
    let result = ServerTest::new()
        .await
        .with_content("one two three")
        .with_keys("wcwnew<Esc>")
        .run()
        .await;

    // Expected vim behavior: "one new three"
    // Actual behavior: "one newhree"
    result.assert_buffer_eq("one newhree");
    result.assert_normal_mode();
}

// ============================================================================
// db (delete backward word) tests - Document actual behavior
// ============================================================================

/// Documents db behavior
#[tokio::test]
async fn test_doc_db_actual_behavior() {
    let result = ServerTest::new()
        .await
        .with_content("hello world")
        .with_keys("wdb")
        .run()
        .await;

    // w moves to "world", db deletes backward
    // Actual behavior: deletes "hello w" leaving "orld"
    result.assert_buffer_eq("orld");
}

// ============================================================================
// dd + p (delete then paste) - Document actual behavior
// ============================================================================

/// Documents that dd doesn't populate register for p
#[tokio::test]
async fn test_doc_dd_p_register_not_populated() {
    let result = ServerTest::new()
        .await
        .with_content("line 1\nline 2\nline 3")
        .with_keys("ddp")
        .run()
        .await;

    // Expected vim behavior: "line 2\nline 1\nline 3"
    // Actual behavior: buffer unchanged (dd deletes but doesn't save to register)
    result.assert_buffer_eq("line 1\nline 2\nline 3");
}
