//! Manual verification for Issue #474 bug fixes.
//!
//! This test exercises the full gRPC stack end-to-end with detailed
//! diagnostic output for visual inspection. Run with:
//!
//! ```bash
//! cargo test -p reovim-client-tui --test manual_verify_474 -- --nocapture
//! ```
//!
//! It verifies all 4 fixes:
//! 1. Bidirectional cursor visibility (one-way presence bug)
//! 2. Cursor labels preserve content (`apply_style` vs `write_str`)
//! 3. Resize isolation between clients
//! 4. Cursor independence (no cross-client following)

#![allow(clippy::items_after_statements)]
#![allow(clippy::uninlined_format_args)]

use std::time::Duration;

use {
    reovim_client_tui::{TuiAppError, TuiHandle, connect_headless},
    reovim_testing::TestServerHarness,
};

async fn headless_tui(addr: &str, width: u16, height: u16) -> Result<TuiHandle, TuiAppError> {
    let (mut app, handle) =
        connect_headless(addr, width, height, None, None, &std::collections::HashSet::new())
            .await?;
    tokio::spawn(async move { app.run().await });
    Ok(handle)
}

fn separator(title: &str) {
    let bar = "=".repeat(72);
    eprintln!();
    eprintln!("{bar}");
    eprintln!("  {title}");
    eprintln!("{bar}");
}

fn print_frame(label: &str, frame: &str) {
    eprintln!();
    eprintln!("--- {label} ---");
    for (i, line) in frame.lines().enumerate() {
        eprintln!("  {:>2}| {}", i + 1, line);
    }
    eprintln!("--- end ---");
}

/// Count unique CBF-8 RGB background colors in an ANSI frame.
fn count_unique_bg_colors(ansi: &str) -> usize {
    let mut colors = std::collections::HashSet::new();
    // Match 48;2;R;G;B patterns (RGB background)
    let bytes = ansi.as_bytes();
    let pattern = b"48;2;";
    for i in 0..bytes.len().saturating_sub(pattern.len()) {
        if &bytes[i..i + pattern.len()] == pattern {
            // Extract R;G;B (up to next 'm' or non-digit/semicolon)
            let rest = &ansi[i + pattern.len()..];
            if let Some(end) = rest.find('m') {
                let rgb = &rest[..end];
                colors.insert(rgb.to_string());
            }
        }
    }
    colors.len()
}

/// Extract cursor position from statusline (pattern: "N:M").
fn extract_cursor_from_statusline(frame: &str) -> Option<(u32, u32)> {
    let last_line = frame.lines().last()?;
    for segment in last_line.split('|') {
        let trimmed = segment.trim();
        if let Some(colon_idx) = trimmed.rfind(':') {
            let before = trimmed[..colon_idx].trim();
            let after = trimmed[colon_idx + 1..].trim();
            if let Some(line_str) = before.split_whitespace().last()
                && let (Ok(line), Ok(col)) = (line_str.parse::<u32>(), after.parse::<u32>())
            {
                return Some((line, col));
            }
        }
    }
    None
}

#[tokio::test]
#[ignore = "pre-existing post-Plan-14-E6: see #758 (render pipeline regression)"]
#[allow(clippy::too_many_lines)]
async fn manual_verification_474() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("Failed to spawn server");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut passed = 0u32;
    let mut failed = 0u32;

    macro_rules! check {
        ($cond:expr, $pass_msg:expr, $fail_msg:expr) => {
            if $cond {
                eprintln!("  PASS: {}", $pass_msg);
                passed += 1;
            } else {
                eprintln!("  FAIL: {}", $fail_msg);
                failed += 1;
            }
        };
    }

    // ========================================================================
    separator("PHASE 0: Setup — Connect 2 TUI Clients");
    // ========================================================================

    let tui_a = headless_tui(&addr, 80, 24)
        .await
        .expect("TUI A failed to connect");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let tui_b = headless_tui(&addr, 60, 20)
        .await
        .expect("TUI B failed to connect");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Add multi-line content from TUI A
    tui_a
        .send_keys("iAlpha line<CR>Bravo line<CR>Charlie line<CR>Delta line<CR>Echo line<Esc>")
        .await
        .expect("Failed to add content");
    tokio::time::sleep(Duration::from_millis(300)).await;

    eprintln!("  Clients connected. Content loaded.");

    // ========================================================================
    separator("TEST 1: Bidirectional Cursor Visibility (Bug #1)");
    // ========================================================================
    eprintln!("  Bug: A could not see B's cursor after B joined.");
    eprintln!("  Fix: Server sets buffer_id on ClientPresence before broadcasting.");

    // Move A to line 3 (Charlie)
    tui_a.send_keys("gg2j").await.expect("A move failed");
    // Move B to line 5 (Echo)
    tui_b.send_keys("gg4j").await.expect("B move failed");
    tokio::time::sleep(Duration::from_millis(400)).await;

    // Capture ANSI from both
    let ansi_a = tui_a.capture("ansi").await.expect("A ansi capture failed");
    let ansi_b = tui_b.capture("ansi").await.expect("B ansi capture failed");

    let colors_in_a = count_unique_bg_colors(&ansi_a);
    let colors_in_b = count_unique_bg_colors(&ansi_b);

    eprintln!("  Unique CBF-8 bg colors in A's view: {colors_in_a}");
    eprintln!("  Unique CBF-8 bg colors in B's view: {colors_in_b}");

    check!(
        ansi_b.contains("48;2;"),
        "B sees A's cursor (B->A, always worked via peers_v2)",
        "B does NOT see A's cursor colors"
    );

    check!(
        ansi_a.contains("48;2;"),
        "A sees B's cursor (A->B, THE BUG FIX — PresenceJoined now has buffer_id)",
        "A does NOT see B's cursor (Bug #1 NOT FIXED)"
    );

    // Print plain text frames for visual inspection
    let plain_a = tui_a.capture("plain_text").await.expect("A plain failed");
    let plain_b = tui_b.capture("plain_text").await.expect("B plain failed");
    print_frame("TUI A (80x24) — cursor on Charlie, should see B's cursor on Echo", &plain_a);
    print_frame("TUI B (60x20) — cursor on Echo, should see A's cursor on Charlie", &plain_b);

    // ========================================================================
    separator("TEST 2: Cursor Labels Preserve Content (Label Fix)");
    // ========================================================================
    eprintln!("  Bug: Labels used write_str() which replaced buffer characters.");
    eprintln!("  Fix: Changed to apply_style() — only overlays bg+fg color.");

    // A is on line 3. Label renders on line 2 (Bravo line) as colored overlay.
    // B should still see "Bravo line" in plain_text capture.

    check!(
        plain_b.contains("Alpha line"),
        "B sees 'Alpha line' (line 1, no overlap)",
        "'Alpha line' missing from B's view"
    );
    check!(
        plain_b.contains("Bravo line"),
        "B sees 'Bravo line' (LABEL OVERLAY LINE — content preserved via apply_style)",
        "'Bravo line' HIDDEN by label (Bug NOT fixed — still using write_str)"
    );
    check!(
        plain_b.contains("Charlie line"),
        "B sees 'Charlie line' (cursor line)",
        "'Charlie line' missing from B's view"
    );
    check!(
        plain_b.contains("Delta line"),
        "B sees 'Delta line' (line 4, no overlap)",
        "'Delta line' missing from B's view"
    );
    check!(
        plain_b.contains("Echo line"),
        "B sees 'Echo line' (B's own cursor line)",
        "'Echo line' missing from B's view"
    );

    // ========================================================================
    separator("TEST 3: Resize Isolation (Bug #3)");
    // ========================================================================
    eprintln!("  Bug: Resizing any client resized ALL clients.");
    eprintln!("  Fix: Added target_client_id to ResizeRequestPayload.");

    // Capture B's frame dimensions BEFORE A resizes
    let b_pre = tui_b
        .capture("plain_text")
        .await
        .expect("B pre-capture failed");
    let b_lines_pre = b_pre.lines().count();
    eprintln!("  B lines before A's resize: {b_lines_pre}");

    // Resize A to 120x40
    tui_a.resize(120, 40).await.expect("A resize failed");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Capture B's dimensions AFTER A resizes
    let b_post = tui_b
        .capture("plain_text")
        .await
        .expect("B post-capture failed");
    let b_lines_post = b_post.lines().count();
    eprintln!("  B lines after  A's resize: {b_lines_post}");

    check!(
        b_lines_pre == b_lines_post,
        format!("B's viewport unchanged ({b_lines_pre} lines both times) — resize isolation works"),
        format!(
            "B's viewport changed from {b_lines_pre} to {b_lines_post} lines (Bug #3 NOT FIXED)"
        )
    );

    // Verify A DID resize
    let a_resized = tui_a
        .capture("plain_text")
        .await
        .expect("A resized capture failed");
    let a_lines_post = a_resized.lines().count();
    eprintln!("  A lines after resize: {a_lines_post}");

    check!(
        a_lines_post > b_lines_post,
        format!("A has more lines ({a_lines_post}) than B ({b_lines_post}) — resize took effect"),
        format!("A doesn't have more lines than B (resize may not have worked)")
    );

    print_frame("TUI B after A's resize (should be unchanged at 60x20)", &b_post);

    // ========================================================================
    separator("TEST 4: Cursor Independence (Bug #2)");
    // ========================================================================
    eprintln!("  Scenario: Moving B's cursor should NOT change A's cursor position.");

    // Resize A back to 80x24 for consistent testing
    tui_a.resize(80, 24).await.expect("A resize back failed");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Position A at line 1, col 2 (0-indexed: 0, 1)
    tui_a.send_keys("ggl").await.expect("A move to 1:2 failed");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let frame_a_pre = tui_a
        .capture("plain_text")
        .await
        .expect("A pre capture failed");
    let cursor_a_pre = extract_cursor_from_statusline(&frame_a_pre);
    eprintln!("  A's cursor BEFORE B moves: {:?}", cursor_a_pre);

    // Move B to a very different position
    tui_b.send_keys("gg4j$").await.expect("B move failed");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let frame_a_post = tui_a
        .capture("plain_text")
        .await
        .expect("A post capture failed");
    let cursor_a_post = extract_cursor_from_statusline(&frame_a_post);
    eprintln!("  A's cursor AFTER  B moves: {:?}", cursor_a_post);

    check!(
        cursor_a_pre == cursor_a_post,
        format!("A's cursor unchanged at {:?} — cursor isolation verified", cursor_a_pre),
        format!(
            "A's cursor changed from {:?} to {:?} (cursor following bug)",
            cursor_a_pre, cursor_a_post
        )
    );

    // ========================================================================
    separator("TEST 5: Late Joiner — 3-Client Scenario");
    // ========================================================================
    eprintln!("  Scenario: Third client joins late. All 3 should see each other.");

    let tui_c = headless_tui(&addr, 80, 24)
        .await
        .expect("TUI C failed to connect");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Move C to line 2
    tui_c.send_keys("gg1j").await.expect("C move failed");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // C should see the buffer content
    let plain_c = tui_c.capture("plain_text").await.expect("C plain failed");
    check!(
        plain_c.contains("Alpha line"),
        "C (late joiner) sees shared buffer content",
        "C cannot see buffer content"
    );

    // C should see remote cursor colors (from A and B)
    let ansi_c = tui_c.capture("ansi").await.expect("C ansi failed");
    let colors_in_c = count_unique_bg_colors(&ansi_c);
    eprintln!("  Unique CBF-8 bg colors in C's view: {colors_in_c}");

    check!(
        colors_in_c >= 2,
        format!("C sees cursors from BOTH A and B ({colors_in_c} unique colors)"),
        format!("C only sees {colors_in_c} remote cursor color(s) — expected >= 2")
    );

    // A should see BOTH B and C's cursors
    let ansi_a_final = tui_a.capture("ansi").await.expect("A final ansi failed");
    let colors_in_a_final = count_unique_bg_colors(&ansi_a_final);
    eprintln!("  Unique CBF-8 bg colors in A's view: {colors_in_a_final}");

    check!(
        colors_in_a_final >= 2,
        format!("A sees cursors from BOTH B and C ({colors_in_a_final} unique colors)"),
        format!("A only sees {colors_in_a_final} remote cursor color(s) — expected >= 2")
    );

    print_frame("TUI C (late joiner, 80x24) — should see A and B cursors", &plain_c);

    // ========================================================================
    separator("TEST 6: Content Integrity After All Operations");
    // ========================================================================

    // Final capture from all 3 clients
    let final_a = tui_a.capture("plain_text").await.expect("final A failed");
    let final_b = tui_b.capture("plain_text").await.expect("final B failed");
    let final_c = tui_c.capture("plain_text").await.expect("final C failed");

    for (name, frame) in [("A", &final_a), ("B", &final_b), ("C", &final_c)] {
        let has_all = frame.contains("Alpha line")
            && frame.contains("Bravo line")
            && frame.contains("Charlie line")
            && frame.contains("Delta line")
            && frame.contains("Echo line");
        check!(
            has_all,
            format!("Client {name} sees all 5 lines of content"),
            format!("Client {name} missing some content lines")
        );
    }

    // ========================================================================
    separator("CLEANUP");
    // ========================================================================

    tui_a.stop().await;
    tui_b.stop().await;
    tui_c.stop().await;

    // ========================================================================
    separator("RESULTS");
    // ========================================================================
    let total = passed + failed;
    eprintln!();
    eprintln!("  Passed:  {passed} / {total}");
    eprintln!("  Failed:  {failed} / {total}");
    eprintln!();

    if failed == 0 {
        eprintln!("  ALL FIXES VERIFIED SUCCESSFULLY");
    } else {
        eprintln!("  SOME FIXES FAILED VERIFICATION");
    }
    eprintln!();

    assert_eq!(failed, 0, "{failed} verification checks failed");
}
