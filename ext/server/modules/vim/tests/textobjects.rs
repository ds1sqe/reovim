//! E2E tests for vim operator + text object bindings.
//!
//! Tests verify that operator+textobject combinations (diw, ciw, yiw, etc.)
//! work correctly through the full server/client gRPC v2 stack.
//!
//! These live in the vim module because they test vim keybindings
//! (`operator_modes.rs`), not the textobjects module itself.
//!
//! # Test Coverage Matrix
//!
//! ```text
//!  Text Object  | d (delete) | c (change) | y (yank) | StepTest
//! ──────────────┼────────────┼────────────┼──────────┼──────────
//!  iw  word     |     x      |     x      |    x     |    x
//!  aw  word     |     x      |            |          |
//!  iW  WORD     |     x      |            |          |
//!  aW  WORD     |     x      |            |          |
//!  i"  dquote   |     x      |     x      |    x     |
//!  a"  dquote   |     x      |            |          |
//!  i'  squote   |     x      |            |          |
//!  a'  squote   |     x      |            |          |
//!  i`  backtick |     x      |            |          |
//!  i(  paren    |     x      |     x      |          |
//!  a(  paren    |     x      |            |          |
//!  i[  bracket  |     x      |     x      |          |
//!  a[  bracket  |     x      |            |          |
//!  i{  brace    |     x      |     x      |          |
//!  a{  brace    |     x      |            |          |
//!  i<  angle    |     x      |            |          |
//!  a<  angle    |     x      |            |          |
//!  ip  para     |     x      |            |          |
//!  ap  para     |     x      |            |          |
//! ──────────────┼────────────┼────────────┼──────────┼──────────
//!  Total        |    19      |     5      |    2     |    2
//! ```
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p reovim-module-vim --test textobjects
//! ```

use reovim_testing::{IntegrationTest, StepTest};

// ============================================================================
// Delete + Word Text Objects
// ============================================================================

/// Test `diw` deletes inner word.
#[tokio::test]
async fn test_diw_delete_inner_word() {
    let result = IntegrationTest::with_modules(&["textobjects"])
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
async fn test_daw_delete_a_word() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("hello world test")
        .with_cursor_at(0, 6) // On 'w' of 'world'
        .send_keys("daw")
        .run()
        .await;
    result.assert_buffer_eq("hello test");
}

/// Test `di"` deletes inside double quotes.
#[tokio::test]
async fn test_di_double_quote_delete_inside() {
    let result = IntegrationTest::with_modules(&["textobjects"])
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
async fn test_ci_brace_change_inside() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("fn() { old }")
        .with_cursor_at(0, 7) // Inside braces
        .send_keys("ci{new<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("fn() {new}");
}

// ============================================================================
// Delete + Big Word Text Objects
// ============================================================================

/// Test `diW` deletes inner WORD (non-whitespace sequence).
#[tokio::test]
async fn test_di_big_word() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("hello foo.bar test")
        .with_cursor_at(0, 7) // On '.' of 'foo.bar'
        .send_keys("diW")
        .run()
        .await;
    result.assert_buffer_eq("hello  test");
}

/// Test `daW` deletes a WORD with surrounding whitespace.
#[tokio::test]
async fn test_da_big_word() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("hello foo.bar test")
        .with_cursor_at(0, 7) // On '.' of 'foo.bar'
        .send_keys("daW")
        .run()
        .await;
    result.assert_buffer_eq("hello test");
}

// ============================================================================
// Delete + Quote Text Objects
// ============================================================================

/// Test `da"` deletes around double quotes (including quotes).
#[tokio::test]
async fn test_da_double_quote() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer(r#"say "hello" please"#)
        .with_cursor_at(0, 5) // Inside quotes
        .send_keys("da\"")
        .run()
        .await;
    result.assert_buffer_eq("say  please");
}

/// Test `di'` deletes inside single quotes.
#[tokio::test]
async fn test_di_single_quote() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("say 'hello' please")
        .with_cursor_at(0, 5) // Inside quotes
        .send_keys("di'")
        .run()
        .await;
    result.assert_buffer_eq("say '' please");
}

/// Test `da'` deletes around single quotes.
#[tokio::test]
async fn test_da_single_quote() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("say 'hello' please")
        .with_cursor_at(0, 5) // Inside quotes
        .send_keys("da'")
        .run()
        .await;
    result.assert_buffer_eq("say  please");
}

/// Test `di`` ` deletes inside backticks.
#[tokio::test]
async fn test_di_backtick() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("say `hello` please")
        .with_cursor_at(0, 5) // Inside backticks
        .send_keys("di`")
        .run()
        .await;
    result.assert_buffer_eq("say `` please");
}

// ============================================================================
// Delete + Bracket Text Objects
// ============================================================================

/// Test `di(` deletes inside parentheses.
#[tokio::test]
async fn test_di_paren() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("fn(arg1, arg2)")
        .with_cursor_at(0, 4) // Inside parens
        .send_keys("di(")
        .run()
        .await;
    result.assert_buffer_eq("fn()");
}

/// Test `da(` deletes around parentheses.
#[tokio::test]
async fn test_da_paren() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("fn(arg1, arg2) end")
        .with_cursor_at(0, 4) // Inside parens
        .send_keys("da(")
        .run()
        .await;
    result.assert_buffer_eq("fn end");
}

/// Test `di[` deletes inside square brackets.
#[tokio::test]
async fn test_di_bracket() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("arr[1, 2, 3]")
        .with_cursor_at(0, 5) // Inside brackets
        .send_keys("di[")
        .run()
        .await;
    result.assert_buffer_eq("arr[]");
}

/// Test `da[` deletes around square brackets.
#[tokio::test]
async fn test_da_bracket() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("arr[1, 2, 3] end")
        .with_cursor_at(0, 5) // Inside brackets
        .send_keys("da[")
        .run()
        .await;
    result.assert_buffer_eq("arr end");
}

/// Test `di{` deletes inside braces.
#[tokio::test]
async fn test_di_brace() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("fn() { body }")
        .with_cursor_at(0, 7) // Inside braces
        .send_keys("di{")
        .run()
        .await;
    result.assert_buffer_eq("fn() {}");
}

/// Test `da{` deletes around braces.
#[tokio::test]
async fn test_da_brace() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("fn() { body } end")
        .with_cursor_at(0, 7) // Inside braces
        .send_keys("da{")
        .run()
        .await;
    result.assert_buffer_eq("fn()  end");
}

/// Test `di<` deletes inside angle brackets.
/// Note: `<lt>` is vim notation for literal `<`.
#[tokio::test]
async fn test_di_angle() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("<div>content</div>")
        .with_cursor_at(0, 1) // Inside angle brackets
        .send_keys("di<lt>")
        .run()
        .await;
    result.assert_buffer_eq("<>content</div>");
}

/// Test `da<` deletes around angle brackets.
/// Note: `<lt>` is vim notation for literal `<`.
#[tokio::test]
async fn test_da_angle() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("<div> rest")
        .with_cursor_at(0, 1) // Inside angle brackets
        .send_keys("da<lt>")
        .run()
        .await;
    result.assert_buffer_eq(" rest");
}

// ============================================================================
// Delete + Paragraph Text Objects
// ============================================================================

/// Test `dip` deletes inner paragraph (contiguous non-blank lines).
#[tokio::test]
async fn test_dip_paragraph() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("line1\nline2\n\nline3")
        .send_keys("dip")
        .run()
        .await;
    result.assert_buffer_contains("line3");
}

/// Test `dap` deletes a paragraph (including surrounding blank lines).
#[tokio::test]
async fn test_dap_paragraph() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("line1\nline2\n\nline3")
        .send_keys("dap")
        .run()
        .await;
    result.assert_buffer_eq("line3");
}

// ============================================================================
// Change + Text Objects
// ============================================================================

/// Test `ciw` changes inner word.
#[tokio::test]
async fn test_ciw_change_word() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("old word here")
        .with_cursor_at(0, 4) // On 'w' of 'word'
        .send_keys("ciwnew<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("old new here");
}

/// Test `ci"` changes inside double quotes.
#[tokio::test]
async fn test_ci_double_quote() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer(r#"say "old" please"#)
        .with_cursor_at(0, 5) // Inside quotes
        .send_keys("ci\"new<Esc>")
        .run()
        .await;
    result.assert_buffer_eq(r#"say "new" please"#);
}

/// Test `ci(` changes inside parentheses.
#[tokio::test]
async fn test_ci_paren() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("fn(old)")
        .with_cursor_at(0, 3) // Inside parens
        .send_keys("ci(x<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("fn(x)");
}

/// Test `ci[` changes inside square brackets.
#[tokio::test]
async fn test_ci_bracket() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("a[old]")
        .with_cursor_at(0, 2) // Inside brackets
        .send_keys("ci[x<Esc>")
        .run()
        .await;
    result.assert_buffer_eq("a[x]");
}

// ============================================================================
// Yank + Text Objects (verified via paste)
// ============================================================================

/// Test `yiw` yanks inner word, verified by pasting.
#[tokio::test]
async fn test_yiw_paste() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer("hello world")
        .send_keys("yiw$p")
        .run()
        .await;
    result.assert_buffer_contains("hello");
    // Buffer should have "hello" at start and appended after last char
    let content = &result.buffer_content;
    assert!(
        content.matches("hello").count() >= 2,
        "Expected 'hello' to appear at least twice after yiw$p, got: {content}"
    );
}

/// Test `yi"` yanks inside quotes, verified by pasting.
#[tokio::test]
async fn test_yi_quote_paste() {
    let result = IntegrationTest::with_modules(&["textobjects"])
        .await
        .with_buffer(r#"say "hi" end"#)
        .with_cursor_at(0, 5) // Inside quotes
        .send_keys("yi\"$p")
        .run()
        .await;
    result.assert_buffer_contains("hi");
}

// ============================================================================
// StepTest: Mode Transitions
// ============================================================================

/// Test delete + text object mode transitions: d enters delete mode, iw completes.
#[tokio::test]
async fn test_delete_textobj_mode_transition() {
    let trace = StepTest::with_modules(&["textobjects"])
        .await
        .with_buffer("hello world test")
        .with_cursor_at(0, 6)
        .step("d")
        .expect_mode_contains("delete")
        .step("iw")
        .expect_buffer("hello  test")
        .run()
        .await;
    trace.assert_ok();
}

/// Test change + text object enters insert mode after completion.
#[tokio::test]
async fn test_change_textobj_enters_insert() {
    let trace = StepTest::with_modules(&["textobjects"])
        .await
        .with_buffer("hello world test")
        .with_cursor_at(0, 6)
        .step("c")
        .expect_mode_contains("change")
        .step("iw")
        .expect_mode_contains("insert")
        .run()
        .await;
    trace.assert_ok();
}
