//! Search integration tests (/, ?, n, N, *, #).
//!
//! **Status**: 2 tests enabled, 13 tests require additional features.
//! - 10 tests require command-line mode for `/` and `?` search patterns (#338)
//! - 3 tests require `*` and `#` word search implementation (#385)
//!
//! Run `cargo test --ignored` to run the remaining ignored tests.

use runner::testing::IntegrationTest;

// ============================================================================
// FORWARD SEARCH (/)
// ============================================================================

#[tokio::test]
#[ignore = "requires command-line mode for / search (#338)"]
async fn test_search_forward_basic() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("/world<Enter>")
        .run()
        .await;
    result.assert_cursor(0, 6);
}

#[tokio::test]
#[ignore = "requires command-line mode for / search (#338)"]
async fn test_search_forward_multiline() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line one\nline two\nline three")
        .send_keys("/three<Enter>")
        .run()
        .await;
    result.assert_cursor(2, 5);
}

#[tokio::test]
#[ignore = "requires command-line mode for / search (#338)"]
async fn test_search_forward_wrap() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world hello")
        .send_keys("$")
        .send_keys("/hello<Enter>")
        .run()
        .await;
    // Should wrap to beginning and find first "hello"
    result.assert_cursor(0, 0);
}

// ============================================================================
// BACKWARD SEARCH (?)
// ============================================================================

#[tokio::test]
#[ignore = "requires command-line mode for ? search (#338)"]
async fn test_search_backward_basic() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world hello")
        .send_keys("$")
        .send_keys("?hello<Enter>")
        .run()
        .await;
    // Should find "hello" at column 12 (second occurrence, before cursor)
    result.assert_cursor(0, 12);
}

#[tokio::test]
#[ignore = "requires command-line mode for ? search (#338)"]
async fn test_search_backward_multiline() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line one\nline two\nline three")
        .send_keys("G$")
        .send_keys("?one<Enter>")
        .run()
        .await;
    result.assert_cursor(0, 5);
}

// ============================================================================
// NEXT/PREVIOUS SEARCH (n, N)
// ============================================================================

#[tokio::test]
#[ignore = "requires command-line mode for / search (#338)"]
async fn test_search_next() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("foo bar foo baz foo")
        .send_keys("/foo<Enter>")
        .send_keys("n")
        .run()
        .await;
    // First search lands on "foo" at 0, n moves to next "foo" at 8
    result.assert_cursor(0, 8);
}

#[tokio::test]
#[ignore = "requires command-line mode for / search (#338)"]
async fn test_search_next_multiple() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("foo bar foo baz foo")
        .send_keys("/foo<Enter>")
        .send_keys("nn")
        .run()
        .await;
    // First search lands on "foo" at 0, nn moves to "foo" at 16
    result.assert_cursor(0, 16);
}

#[tokio::test]
#[ignore = "requires command-line mode for / search (#338)"]
async fn test_search_previous() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("foo bar foo baz foo")
        .send_keys("$")
        .send_keys("/foo<Enter>")
        .send_keys("N")
        .run()
        .await;
    // Search wraps to beginning "foo" at 0, N goes back to last "foo" at 16
    result.assert_cursor(0, 16);
}

// ============================================================================
// WORD SEARCH (*, #)
// ============================================================================

#[tokio::test]
#[ignore = "requires search implementation via SessionContext (#385)"]
async fn test_search_word_forward() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world hello")
        .send_keys("*")
        .run()
        .await;
    // * on "hello" should find next "hello" at column 12
    result.assert_cursor(0, 12);
}

#[tokio::test]
#[ignore = "requires search implementation via SessionContext (#385)"]
async fn test_search_word_backward() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world hello")
        .send_keys("$")
        .send_keys("#")
        .run()
        .await;
    // # on "hello" at end should find previous "hello" at column 0
    result.assert_cursor(0, 0);
}

#[tokio::test]
#[ignore = "requires search implementation via SessionContext (#385)"]
async fn test_search_word_with_n() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("foo bar foo baz foo")
        .send_keys("*")
        .send_keys("n")
        .run()
        .await;
    // * on "foo" finds second "foo" at 8, n finds third "foo" at 16
    result.assert_cursor(0, 16);
}

// ============================================================================
// SEARCH CANCEL
// ============================================================================

#[tokio::test]
#[ignore = "requires command-line mode for / search (#338)"]
async fn test_search_cancel_with_escape() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("/world<Escape>")
        .run()
        .await;
    // Cursor should remain at original position
    result.assert_cursor(0, 0);
}

// ============================================================================
// REGEX SEARCH
// ============================================================================

#[tokio::test]
#[ignore = "requires command-line mode for / search (#338)"]
async fn test_search_regex_pattern() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello123 world456")
        .send_keys("/\\d+<Enter>")
        .run()
        .await;
    // Should find first digit sequence at column 5
    result.assert_cursor(0, 5);
}

#[tokio::test]
async fn test_search_regex_word_boundary() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("the then they")
        .send_keys("/\\bthe\\b<Enter>")
        .run()
        .await;
    // Should find exact "the" at column 0, not "then" or "they"
    result.assert_cursor(0, 0);
}

// ============================================================================
// SEARCH NOT FOUND
// ============================================================================

#[tokio::test]
async fn test_search_not_found() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("/xyz<Enter>")
        .run()
        .await;
    // Cursor should remain at original position
    result.assert_cursor(0, 0);
}
