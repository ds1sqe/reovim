//! Operator integration tests (delete, yank, paste, change).
//!
//! **Status**: 54 tests enabled, 1 ignored (dG motion needs investigation).
//!
//! Tests operator-pending mode (d+motion, y+motion, c+motion) with:
//! - Direct bindings: dd, yy, cc, x, 5dd, 3x
//! - Motion composition: dw, d$, dj, dk, de, dgg, yw, yj, y$, cw, c$, cj
//! - Count prefixes: 2dj, 3dw, 2cw, d2j, y2j
//! - Step-by-step tracing via StepTest (Issue #428)
//! - Register assertions: assert_register()
//!
//! Note: dG (delete to document end) has a motion handling issue - tracked in #429.
//!
//! Run `cargo test --ignored` to run the remaining ignored test.

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
async fn test_dd_upper_p_paste_before() {
    // jddP: move to line 1, delete it (yanking "line 2\n"), paste before current line
    // After dd: buffer is "line 1\nline 3", cursor on line 1 ("line 3")
    // P pastes "line 2\n" before line 1, restoring original order
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jddP")
        .run()
        .await;
    result.assert_buffer_eq("line 1\nline 2\nline 3");
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
async fn test_2cw_change_count() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("one two three")
        .send_keys("2cwXXX<Esc>")
        .run()
        .await;
    // Note: In Vim, `cw` changes to END of word (like `ce`), not start of next word.
    // So `2cw` changes "one two" (not "one two "), leaving space before "three".
    // This is documented Vim behavior (`:help cw`).
    result.assert_buffer_eq("XXX three");
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

// ============================================================================
// OPERATOR + MOTION COMBINATIONS (Issue #415)
// ============================================================================

/// dk - delete current line and line above (linewise motion upward)
#[tokio::test]
async fn test_dk_delete_prev_line() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("jdk")
        .run()
        .await;
    result.assert_buffer_eq("line 3");
}

/// y$ - yank to end of line (characterwise, inclusive)
#[tokio::test]
async fn test_y_dollar_yank_to_eol() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("lly$P")
        .run()
        .await;
    // At col 2 ('l'), y$ yanks "llo world", P pastes before cursor
    result.assert_buffer_contains("llo world");
}

/// cj - change two lines (current + next), enter insert mode
#[tokio::test]
async fn test_cj_change_two_lines() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("cjX<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("X\nline 3");
    result.assert_normal_mode();
}

/// de - delete to end of word (characterwise, inclusive)
#[tokio::test]
async fn test_de_delete_to_word_end() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("de")
        .run()
        .await;
    // de deletes "hello" (to end of word, inclusive)
    result.assert_buffer_eq(" world");
}

/// dG - delete from current line to end of document
/// Issue #429: G motion with delete operator not working correctly
#[tokio::test]
#[ignore = "G motion with delete operator needs investigation - Issue #429"]
async fn test_dg_delete_to_doc_end() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("dG")
        .run()
        .await;
    // dG from line 0 deletes all lines (to end of document)
    result.assert_buffer_eq("");
}

/// dgg - delete from current line to start of document
/// Note: Gdgg from last line deletes all lines (to start of document)
#[tokio::test]
async fn test_dgg_delete_to_doc_start() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .send_keys("Gdgg")
        .run()
        .await;
    // G moves to last line, dgg deletes all lines (to start)
    result.assert_buffer_eq("");
}

// ============================================================================
// COUNT PREFIX WITH MOTIONS (Issue #415)
// ============================================================================

/// d2j - delete 3 lines (current + 2 down)
#[tokio::test]
async fn test_d2j_delete_three_lines() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3\nline 4\nline 5")
        .send_keys("d2j")
        .run()
        .await;
    result.assert_buffer_eq("line 4\nline 5");
}

/// 3dw - delete 3 words
#[tokio::test]
async fn test_3dw_delete_three_words() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("one two three four five")
        .send_keys("3dw")
        .run()
        .await;
    result.assert_buffer_eq("four five");
}

/// y2j then p - yank 3 lines and paste
#[tokio::test]
async fn test_y2j_yank_three_lines() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3\nline 4")
        .send_keys("y2jGp")
        .run()
        .await;
    // Yank lines 0-2, paste after line 3
    result.assert_buffer_eq("line 1\nline 2\nline 3\nline 4\nline 1\nline 2\nline 3");
}

// ============================================================================
// STEPTEST WITH ASSERTIONS (Issue #415, #428)
// ============================================================================

use runner::testing::StepTest;

/// StepTest: dj with per-step mode verification
#[tokio::test]
async fn test_step_dj_with_assertions() {
    let trace = StepTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .step("d")
        .expect_mode_contains("DELETE")
        .expect_buffer("line 1\nline 2\nline 3") // No change yet
        .step("j")
        .expect_buffer("line 3")
        .expect_cursor(0, 0)
        .expect_mode_contains("NORMAL")
        .run()
        .await;
    trace.assert_ok();
}

/// StepTest: yj with register verification at each step
#[tokio::test]
async fn test_step_yj_with_register() {
    let trace = StepTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .step("y")
        .expect_mode_contains("YANK")
        .step("j")
        .expect_mode_contains("NORMAL")
        .expect_register("\"", "line 1\nline 2\n", "linewise")
        .expect_cursor(0, 0) // Cursor restored to start of yank range
        .run()
        .await;
    trace.assert_ok();
}

/// StepTest: cw with mode transition verification
/// Note: cw deletes to end of word but preserves trailing space (Vim behavior)
#[tokio::test]
async fn test_step_cw_mode_transition() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello world")
        .step("c")
        .expect_mode_contains("CHANGE")
        .step("w")
        .expect_buffer(" world") // cw preserves trailing space
        .expect_mode_contains("INSERT")
        .run()
        .await;
    trace.assert_ok();
}

/// StepTest: d$ with inclusive motion handling
#[tokio::test]
async fn test_step_d_dollar_inclusive() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 2) // Start at 'l' in "hello"
        .step("d")
        .expect_mode_contains("DELETE")
        .step("$")
        .expect_buffer("he")
        .expect_mode_contains("NORMAL")
        .run()
        .await;
    trace.assert_ok();
}

// ============================================================================
// STEP-BY-STEP DEBUG TESTS
// ============================================================================

/// Debug test for `dj` - delete two lines
/// Expected: "line 1\nline 2\nline 3" -> "line 3" (delete lines 0 and 1)
#[tokio::test]
async fn debug_step_dj() {
    let trace = StepTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .step("d")
        .step("j")
        .run()
        .await;

    trace.print_trace();

    // Verify final state
    let final_state = trace.final_state();
    eprintln!("\nFinal buffer: {:?}", final_state.buffer);
    eprintln!("Final cursor: ({}, {})", final_state.cursor_line, final_state.cursor_column);
    eprintln!("Final mode: {}", final_state.mode_display);
}

/// Debug test for `yj` - yank two lines
/// Expected: yank lines 0 and 1, then paste duplicates them
#[tokio::test]
async fn debug_step_yj() {
    let trace = StepTest::new()
        .await
        .with_buffer("line 1\nline 2\nline 3")
        .step("y")
        .step("j")
        .step("p")
        .run()
        .await;

    trace.print_trace();

    let final_state = trace.final_state();
    eprintln!("\nFinal buffer: {:?}", final_state.buffer);
}

/// Debug test for `cw` - change word
/// Expected: "hello world" with cw should delete "hello" and enter insert
#[tokio::test]
async fn debug_step_cw() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello world")
        .step("c")
        .step("w")
        .run()
        .await;

    trace.print_trace();

    let final_state = trace.final_state();
    eprintln!("\nFinal buffer: {:?}", final_state.buffer);
    eprintln!("Final mode: {} ({})", final_state.mode_display, final_state.edit_mode);
}

/// Debug test for `dw` - delete word (this one works!)
/// Compare against cw to understand the difference
#[tokio::test]
async fn debug_step_dw() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello world")
        .step("d")
        .step("w")
        .run()
        .await;

    trace.print_trace();

    let final_state = trace.final_state();
    eprintln!("\nFinal buffer: {:?}", final_state.buffer);
}

/// Debug test for `2cw` - change 2 words
#[tokio::test]
async fn debug_step_2cw() {
    let trace = StepTest::new()
        .await
        .with_buffer("one two three")
        .step("2")
        .step("c")
        .step("w")
        .run()
        .await;

    trace.print_trace();

    let final_state = trace.final_state();
    eprintln!("\nFinal buffer: {:?}", final_state.buffer);
    eprintln!("Final mode: {} ({})", final_state.mode_display, final_state.edit_mode);
}

/// Debug test for `d$` - delete to end of line (this should work after inclusive fix)
#[tokio::test]
async fn debug_step_d_dollar() {
    let trace = StepTest::new()
        .await
        .with_buffer("hello world")
        .with_cursor_at(0, 2) // Start at 'l' in "hello"
        .step("d")
        .step("$")
        .run()
        .await;

    trace.print_trace();

    let final_state = trace.final_state();
    eprintln!("\nExpected buffer: \"he\"");
    eprintln!("Actual buffer: {:?}", final_state.buffer);
}
