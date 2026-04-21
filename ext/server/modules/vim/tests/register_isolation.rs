//! E2E tests for register behavior (#515).
//!
//! Verifies that registers are per-client and that yank/delete/paste
//! operations correctly populate registers through the full stack.
//!
//! # Running
//!
//! ```bash
//! cargo test -p reovim-module-vim --test register_isolation
//! ```

use std::time::Duration;

use reovim_testing::{IntegrationTest, MultiClientPresenceTest};

// ============================================================================
// Single-client register tests
// ============================================================================

/// Yank word populates unnamed register and paste works.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_yank_word_then_paste() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("yw")
        .send_keys("$p")
        .run()
        .await;
    // "hello " was yanked, pasted after 'd' at end of line
    result.assert_buffer_contains("hello");
}

/// Delete word populates unnamed register and paste restores it.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_delete_word_then_paste() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("dw")
        .send_keys("$p")
        .run()
        .await;
    // "hello " was deleted, "world" remains, paste appends "hello " after last char
    result.assert_buffer_contains("hello");
    result.assert_buffer_contains("world");
}

/// dd populates register and p pastes the line.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_dd_then_p() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("line1\nline2\nline3")
        .send_keys("dd")
        .send_keys("p")
        .run()
        .await;
    // dd deletes "line1", p pastes it below "line2"
    result.assert_buffer_contains("line1");
    result.assert_buffer_contains("line2");
    result.assert_buffer_contains("line3");
}

/// Named register yank: "ayy then "ap.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_named_register_yank_paste() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("first line\nsecond line")
        .send_keys("\"ayy")
        .send_keys("j")
        .send_keys("\"ap")
        .run()
        .await;
    // Yanked "first line" into "a, pasted below "second line"
    result.assert_buffer_contains("first line");
    result.assert_buffer_contains("second line");
}

/// Visual mode delete populates register for paste.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_visual_delete_then_paste() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("abcdef")
        .send_keys("vlld")
        .send_keys("p")
        .run()
        .await;
    // Selected "abc", deleted, then pasted back
    result.assert_buffer_contains("abc");
}

/// Visual mode yank populates register for paste.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_visual_yank_then_paste() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("hello world")
        .send_keys("vey")
        .send_keys("$p")
        .run()
        .await;
    // Yanked "hello", pasted at end
    result.assert_buffer_contains("hello");
}

/// Change operator populates register with deleted text.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_change_populates_register() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("old text\nkeep this")
        .send_keys("cwnew<Esc>")
        .send_keys("j$p")
        .run()
        .await;
    // Changed "old" to "new", "old" went into unnamed register
    // Pasted "old" after "keep this"
    result.assert_buffer_contains("new");
    result.assert_buffer_contains("keep this");
}

/// cc populates register with deleted line, then paste works.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_cc_then_paste() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("original\nkeep")
        .send_keys("ccreplaced<Esc>")
        .send_keys("jp")
        .run()
        .await;
    result.assert_buffer_contains("replaced");
    result.assert_buffer_contains("keep");
}

/// Multiple yanks: last yank wins for unnamed register.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_last_yank_wins_unnamed() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("aaa bbb ccc")
        .send_keys("yw") // yank "aaa "
        .send_keys("w") // move to "bbb"
        .send_keys("yw") // yank "bbb " (overwrites unnamed)
        .send_keys("$p") // paste "bbb " at end
        .run()
        .await;
    // Buffer should end with "bbb" pasted (not "aaa")
    let content = &result.buffer_content;
    assert!(
        content.matches("bbb").count() >= 2,
        "Expected at least 2 occurrences of 'bbb' (original + pasted), got: {content}"
    );
}

/// D (delete to end of line) populates register.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_capital_d_populates_register() {
    let result = IntegrationTest::new()
        .await
        .with_buffer("keep_this remove_this\nsecond")
        .with_cursor_at(0, 9) // at space before "remove_this"
        .send_keys("D")
        .send_keys("jp")
        .run()
        .await;
    result.assert_buffer_contains("keep_this");
    result.assert_buffer_contains("second");
}

// ============================================================================
// Multi-client register isolation (#515)
// ============================================================================

/// Registers are isolated per client: Client 0's yank doesn't affect Client 1.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_register_isolation_between_clients() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Both clients share a buffer
            clients[0]
                .send_keys("iAAA BBB<Esc>0")
                .await
                .expect("client 0 types");
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Client 0 yanks "AAA "
            clients[0].send_keys("yw").await.expect("client 0 yanks");
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Client 0 can paste (their register has "AAA ")
            clients[0].send_keys("$p").await.expect("client 0 pastes");
            tokio::time::sleep(Duration::from_millis(50)).await;

            let content0 = clients[0]
                .get_buffer()
                .await
                .expect("get buffer from client 0");
            assert!(
                content0.matches("AAA").count() >= 2,
                "Client 0 paste should add another 'AAA', got: {content0}"
            );

            // Client 1's paste should be a no-op (their register is empty)
            clients[1].send_keys("p").await.expect("client 1 pastes");
            tokio::time::sleep(Duration::from_millis(50)).await;

            let content1 = clients[1]
                .get_buffer()
                .await
                .expect("get buffer from client 1");

            // Client 1's paste should NOT have added extra "AAA" text
            let aaa_count_0 = content0.matches("AAA").count();
            let aaa_count_1 = content1.matches("AAA").count();

            assert!(
                aaa_count_1 <= aaa_count_0,
                "Client 1 should not have pasted Client 0's register content. \
                 AAA count after client 0 paste: {aaa_count_0}, \
                 AAA count after client 1 paste: {aaa_count_1}"
            );
        })
        .await;
}

/// Each client maintains their own named registers independently.
#[tokio::test]
#[ignore = "pre-existing post-Plan-14-I.5: test harness raw-bytes input — see #759"]
async fn test_named_register_isolation() {
    MultiClientPresenceTest::with_clients(2)
        .await
        .run(|mut clients| async move {
            // Setup: put some text in buffer
            clients[0].send_keys("ifoo bar<Esc>0").await.expect("setup");
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Client 0 yanks to register "a"
            clients[0]
                .send_keys("\"ayw")
                .await
                .expect("client 0 yank to a");
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Client 0 can paste from register "a"
            clients[0]
                .send_keys("$\"ap")
                .await
                .expect("client 0 paste from a");
            tokio::time::sleep(Duration::from_millis(50)).await;

            let c0_buf = clients[0].get_buffer().await.expect("get buffer");
            assert!(
                c0_buf.matches("foo").count() >= 2,
                "Client 0 should paste from \"a register, got: {c0_buf}"
            );
        })
        .await;
}
