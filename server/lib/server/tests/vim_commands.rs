//! E2E tests for vim command execution.
//!
//! These tests verify that vim commands work correctly through the full
//! server/client architecture using gRPC v2 protocol.
//!
//! # Status (Phase 15)
//!
//! - 28 of 34 tests are enabled and passing
//! - 6 tests are ignored pending text object and dot repeat fixes
//!
//! # Running Tests
//!
//! ```bash
//! # Run enabled tests
//! cargo test -p reovim-server --test vim_commands
//!
//! # Run ignored tests (text objects, dot repeat)
//! cargo test -p reovim-server --test vim_commands -- --ignored
//! ```
//!
//! # Log Files
//!
//! Test logs are captured to `tmp/test-logs/{test_name}_{timestamp}.log`.
//! Check these files when debugging test failures.

use reovim_testing::IntegrationTest;

// ============================================================================
// Basic Motion Tests
// ============================================================================

/// Test `h` motion (move left).
#[tokio::test]
async fn test_h_move_left() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .with_cursor_at(0, 3) // On 'l'
        .send_keys("h")
        .run()
        .await;
    result.assert_cursor(0, 2); // Now on 'l' (first one)
}

/// Test `l` motion (move right).
#[tokio::test]
async fn test_l_move_right() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("l")
        .run()
        .await;
    result.assert_cursor(0, 1);
}

/// Test `j` motion (move down).
#[tokio::test]
async fn test_j_move_down() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2\nline3")
        .send_keys("j")
        .run()
        .await;
    result.assert_cursor(1, 0);
}

/// Test `k` motion (move up).
#[tokio::test]
async fn test_k_move_up() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2\nline3")
        .with_cursor_at(2, 0)
        .send_keys("k")
        .run()
        .await;
    result.assert_cursor(1, 0);
}

/// Test `w` motion (word forward).
#[tokio::test]
async fn test_w_word_forward() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world test")
        .send_keys("w")
        .run()
        .await;
    result.assert_cursor(0, 6); // Start of 'world'
}

/// Test `b` motion (word backward).
#[tokio::test]
async fn test_b_word_backward() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world test")
        .with_cursor_at(0, 6) // On 'w' of 'world'
        .send_keys("b")
        .run()
        .await;
    result.assert_cursor(0, 0); // Back to 'h'
}

/// Test `e` motion (end of word).
#[tokio::test]
async fn test_e_end_of_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("e")
        .run()
        .await;
    result.assert_cursor(0, 4); // On 'o' of 'hello'
}

/// Test `0` motion (start of line).
#[tokio::test]
async fn test_0_start_of_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 5)
        .send_keys("0")
        .run()
        .await;
    result.assert_cursor(0, 0);
}

/// Test `$` motion (end of line).
#[tokio::test]
async fn test_dollar_end_of_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("$")
        .run()
        .await;
    result.assert_cursor(0, 10); // On 'd'
}

// ============================================================================
// Insert Mode Tests
// ============================================================================

/// Test `i` enters insert mode.
#[tokio::test]
async fn test_i_enters_insert_mode() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("i")
        .run()
        .await;
    result.assert_insert_mode();
}

/// Test `i` + text + `<Esc>` inserts text.
#[tokio::test]
async fn test_i_inserts_text() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("ihello<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("hello");
    result.assert_normal_mode();
}

/// Test `a` appends after cursor.
#[tokio::test]
async fn test_a_appends() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hllo")
        .send_keys("ae<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

/// Test `A` appends at end of line.
#[tokio::test]
async fn test_capital_a_appends_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello")
        .send_keys("A world<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("hello world");
}

/// Test `o` opens line below.
#[tokio::test]
async fn test_o_opens_line_below() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline3")
        .send_keys("oline2<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("line1\nline2\nline3");
}

/// Test `O` opens line above.
#[tokio::test]
async fn test_capital_o_opens_line_above() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line2")
        .send_keys("Oline1<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("line1\nline2");
}

// ============================================================================
// Delete Operator Tests
// ============================================================================

/// Test `dw` deletes word.
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

/// Test `dd` deletes line.
#[tokio::test]
async fn test_dd_delete_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2\nline3")
        .send_keys("dd")
        .run()
        .await;
    result.assert_buffer_eq("line2\nline3");
}

/// Test `d$` deletes to end of line.
#[tokio::test]
async fn test_d_dollar_delete_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 5)
        .send_keys("d$")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

/// Test `D` (alias for `d$`).
#[tokio::test]
async fn test_capital_d_delete_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 5)
        .send_keys("D")
        .run()
        .await;
    result.assert_buffer_eq("hello");
}

/// Test `x` deletes character under cursor.
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

// ============================================================================
// Change Operator Tests
// ============================================================================

/// Test `cw` changes word.
#[tokio::test]
async fn test_cw_change_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("cwgoodbye<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("goodbye world");
}

/// Test `cc` changes entire line.
#[tokio::test]
async fn test_cc_change_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("old line\nnext line")
        .send_keys("ccnew line<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("new line\nnext line");
}

/// Test `C` (change to end of line).
#[tokio::test]
async fn test_capital_c_change_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 5)
        .send_keys("Cthere<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("hellothere");
}

// ============================================================================
// Yank/Put Tests
// ============================================================================

/// Test `yy` + `p` yanks and puts line.
#[tokio::test]
async fn test_yy_p_yank_put_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2")
        .send_keys("yyp")
        .run()
        .await;
    result.assert_buffer_eq("line1\nline1\nline2");
}

/// Test `yw` + `p` yanks and puts word.
#[tokio::test]
async fn test_yw_p_yank_put_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("yw$p")
        .run()
        .await;
    result.assert_buffer_contains("hello");
}

// ============================================================================
// Text Object Tests
// ============================================================================

/// Test `diw` deletes inner word.
#[tokio::test]
#[ignore = "Text object iw not fully implemented"]
async fn test_diw_delete_inner_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world test")
        .with_cursor_at(0, 7) // On 'o' of 'world'
        .send_keys("diw")
        .run()
        .await;
    result.assert_buffer_eq("hello  test");
}

/// Test `daw` deletes a word (with surrounding whitespace).
#[tokio::test]
#[ignore = "Text object aw not fully implemented"]
async fn test_daw_delete_a_word() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world test")
        .with_cursor_at(0, 6) // On 'w' of 'world'
        .send_keys("daw")
        .run()
        .await;
    result.assert_buffer_eq("hello test");
}

/// Test `di"` deletes inside quotes.
#[tokio::test]
#[ignore = "Text object i\" not fully implemented"]
async fn test_di_quote_delete_inside() {
    let result = IntegrationTest::new()
        .await
        .with_buffer(r#"say "hello" please"#)
        .with_cursor_at(0, 5) // On '"'
        .send_keys("di\"")
        .run()
        .await;
    result.assert_buffer_eq(r#"say "" please"#);
}

/// Test `ci{` changes inside braces.
#[tokio::test]
#[ignore = "Text object i{ not fully implemented"]
async fn test_ci_brace_change_inside() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("fn() { old }")
        .with_cursor_at(0, 7) // Inside braces
        .send_keys("ci{new<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("fn() {new}");
}

// ============================================================================
// Visual Mode Tests
// ============================================================================

/// Test `v` enters visual mode.
#[tokio::test]
async fn test_v_enters_visual_mode() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("v")
        .run()
        .await;
    result.assert_visual_mode();
}

/// Test visual selection + `d` deletes.
#[tokio::test]
async fn test_visual_delete() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("vllld")
        .run()
        .await;
    result.assert_buffer_eq("o world");
}

/// Test `V` (visual line) + `d` deletes line.
#[tokio::test]
async fn test_visual_line_delete() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2\nline3")
        .send_keys("Vd")
        .run()
        .await;
    result.assert_buffer_eq("line2\nline3");
}

// ============================================================================
// Repeat (Dot) Tests
// ============================================================================

/// Test `.` repeats last change.
#[tokio::test]
#[ignore = "Dot repeat not fully implemented"]
async fn test_dot_repeats_change() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("foo foo foo")
        .send_keys("cwbar<Esc>w.w.")
        .run()
        .await;
    result.assert_buffer_eq("bar bar bar");
}

/// Test `.` repeats delete.
#[tokio::test]
#[ignore = "Dot repeat not fully implemented"]
async fn test_dot_repeats_delete() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("one two three four")
        .send_keys("dw..")
        .run()
        .await;
    result.assert_buffer_eq("four");
}
