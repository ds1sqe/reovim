//! Manual multi-client test for extension decoupling (#468).
//!
//! Exercises cmdline popup, whichkey popup, and multi-client isolation
//! against a REAL running server. All scenarios run sequentially to avoid
//! shared-state interference.
//!
//! Usage:
//!   # Start server first:
//!   `./target/release/reovim` server --grpc 13500
//!
//!   # Then run:
//!   cargo test -p reovim-client-tui --test `manual_test_468` -- --nocapture
#![allow(clippy::too_many_lines)]

use std::time::Duration;

use reovim_client_tui::{TuiAppError, TuiHandle, connect_headless};

const ADDR: &str = "127.0.0.1:13500";
const W: u16 = 80;
const H: u16 = 24;

async fn headless(addr: &str) -> Result<TuiHandle, TuiAppError> {
    let (mut app, handle) =
        connect_headless(addr, W, H, None, None, &std::collections::HashSet::new()).await?;
    tokio::spawn(async move { app.run().await });
    Ok(handle)
}

fn dump(label: &str, frame: &str) {
    eprintln!("\n===== {label} =====");
    for (i, line) in frame.lines().enumerate() {
        eprintln!("{i:>2} | {line}");
    }
    eprintln!("===== /{label} =====\n");
}

/// All scenarios run sequentially in one test to avoid parallel race conditions
/// when sharing a single server session.
#[tokio::test]
#[ignore = "requires a running server on port 13500 (#468)"]
async fn manual_test_all_scenarios() {
    // ==================================================================
    // Scenario 1: Cmdline popup lifecycle — activate, type, escape
    // ==================================================================
    eprintln!("\n======================================================================");
    eprintln!("[scenario 1] Cmdline popup full lifecycle");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect failed");
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Baseline (no popup)
        let frame = c1.capture("plain_text").await.unwrap();
        dump("baseline", &frame);
        assert!(!frame.contains('\u{256D}'), "No popup at baseline");

        // Press `:` -> popup appears
        c1.send_keys(":").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(3), |f| f.contains('\u{256D}'))
            .await
            .expect("popup did not appear after `:`");
        dump("after `:` -- popup visible", &frame);
        assert!(frame.contains('\u{256D}'), "Top border");
        assert!(frame.contains('\u{2570}'), "Bottom border");
        let content = frame
            .lines()
            .find(|l| l.contains('\u{2502}') && l.contains(':'))
            .expect("No content row with `:` prompt");
        eprintln!("[ok] Prompt row: {content}");

        // Type "wqa" -> text appears inside popup
        for ch in ['w', 'q', 'a'] {
            c1.send_keys(&ch.to_string()).await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| {
                f.lines()
                    .any(|l| l.contains('\u{2502}') && l.contains("wqa"))
            })
            .await
            .expect("typed text did not appear");
        dump("after typing 'wqa'", &frame);
        let row = frame.lines().find(|l| l.contains("wqa")).unwrap();
        eprintln!("[ok] Input visible: {row}");

        // Escape -> popup disappears
        c1.send_keys("<Esc>").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| !f.contains('\u{256D}'))
            .await
            .expect("popup did not disappear after Escape");
        dump("after Escape -- popup gone", &frame);
        assert!(!frame.contains('\u{256D}'));
        eprintln!("[ok] Popup dismissed");

        c1.stop().await;
        eprintln!("[scenario 1] PASSED\n");
    }

    // ==================================================================
    // Scenario 2: Search prompt `/` + type + escape
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 2] Search cmdline (`/`)");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect failed");
        tokio::time::sleep(Duration::from_millis(300)).await;

        c1.send_keys("/").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(3), |f| {
                f.lines().any(|l| l.contains('\u{2502}') && l.contains('/'))
            })
            .await
            .expect("search popup did not appear");
        dump("after `/` -- search popup", &frame);
        let row = frame
            .lines()
            .find(|l| l.contains('\u{2502}') && l.contains('/'))
            .unwrap();
        assert!(row.contains('/'), "Search prompt `/` visible");
        eprintln!("[ok] Search prompt: {row}");

        // Type search query
        for ch in ['h', 'e', 'l', 'l', 'o'] {
            c1.send_keys(&ch.to_string()).await.unwrap();
        }
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| {
                f.lines()
                    .any(|l| l.contains('\u{2502}') && l.contains("hello"))
            })
            .await
            .expect("search text not visible");
        dump("after typing 'hello'", &frame);

        c1.send_keys("<Esc>").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| !f.contains('\u{256D}'))
            .await
            .expect("search popup did not disappear");
        assert!(!frame.contains('\u{256D}'));
        eprintln!("[ok] Search popup dismissed");

        c1.stop().await;
        eprintln!("[scenario 2] PASSED\n");
    }

    // ==================================================================
    // Scenario 3: Which-key popup — `g` prefix shows hints, escape dismisses
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 3] Which-key popup (`g` prefix) full lifecycle");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect failed");
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Baseline: no popup
        let baseline = c1.capture("plain_text").await.unwrap();
        assert!(!baseline.contains('\u{256D}'), "No popup at baseline for whichkey");

        // Press `g` — which-key appears with hints after resolver returns Pending
        c1.send_keys("g").await.unwrap();

        let frame = c1
            .wait_for(Duration::from_secs(3), |f| {
                // Which-key popup must have box border AND actual hint content
                f.contains('\u{256D}') && f.contains('\u{2570}')
            })
            .await
            .expect("which-key popup did not appear after `g`");
        dump("whichkey popup for `g`", &frame);

        // Verify the popup has box borders
        assert!(frame.contains('\u{256D}'), "Top border present");
        assert!(frame.contains('\u{2570}'), "Bottom border present");

        // Verify prefix title is shown (the prefix "g" appears in the border)
        let has_prefix = frame
            .lines()
            .any(|l| l.contains('\u{256D}') && l.contains('g'))
            || frame.lines().any(|l| l.contains('g'));
        assert!(has_prefix, "Prefix 'g' should be visible");

        // Verify at least some hint content is rendered inside the popup
        let hint_rows: Vec<&str> = frame.lines().filter(|l| l.contains('\u{2502}')).collect();
        eprintln!("[info] Hint rows ({} total):", hint_rows.len());
        for r in &hint_rows {
            eprintln!("  {r}");
        }
        assert!(hint_rows.len() >= 2, "Should have at least 2 hint rows (border sides)");

        // Escape dismisses which-key popup
        c1.send_keys("<Esc>").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| !f.contains('\u{256D}'))
            .await
            .expect("which-key popup did not disappear after Escape");
        dump("after Escape -- whichkey gone", &frame);
        assert!(!frame.contains('\u{256D}'), "Popup gone after Escape");
        eprintln!("[ok] Which-key popup dismissed");

        c1.stop().await;
        eprintln!("[scenario 3] PASSED\n");
    }

    // ==================================================================
    // Scenario 3b: Which-key `z` prefix — scroll hints
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 3b] Which-key popup (`z` prefix)");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect failed");
        tokio::time::sleep(Duration::from_millis(300)).await;

        c1.send_keys("z").await.unwrap();

        let frame = c1
            .wait_for(Duration::from_secs(3), |f| f.contains('\u{256D}') && f.contains('\u{2570}'))
            .await
            .expect("which-key popup did not appear after `z`");
        dump("whichkey popup for `z`", &frame);

        // Popup appeared with border
        assert!(frame.contains('\u{256D}'), "Top border for z popup");
        assert!(frame.contains('\u{2570}'), "Bottom border for z popup");

        // Complete the pending sequence with `z` -> zz (center scroll)
        c1.send_keys("z").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| !f.contains('\u{256D}'))
            .await
            .expect("which-key popup should dismiss after completing `zz`");
        assert!(!frame.contains('\u{256D}'), "Popup gone after completing zz");
        eprintln!("[ok] Which-key popup dismissed after completing `zz`");

        c1.stop().await;
        eprintln!("[scenario 3b] PASSED\n");
    }

    // ==================================================================
    // Scenario 3c: Which-key `g` completion — `gg` moves to top
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 3c] Which-key completion (`gg` goto top)");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect failed");
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Press `g` — which-key popup appears
        c1.send_keys("g").await.unwrap();

        let frame = c1
            .wait_for(Duration::from_secs(3), |f| f.contains('\u{256D}'))
            .await
            .expect("which-key popup did not appear after `g`");
        dump("whichkey popup for `g`", &frame);
        assert!(frame.contains('\u{256D}'), "Top border for g popup");

        // Complete with `g` to execute `gg` (goto top)
        c1.send_keys("g").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| !f.contains('\u{256D}'))
            .await
            .expect("popup should dismiss after completing `gg`");
        dump("after `gg`", &frame);
        assert!(!frame.contains('\u{256D}'), "Popup gone after completing gg");
        eprintln!("[ok] `gg` completed, popup dismissed");

        c1.stop().await;
        eprintln!("[scenario 3c] PASSED\n");
    }

    // ==================================================================
    // Scenario 3d: Which-key multi-client isolation
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 3d] Which-key popup multi-client isolation");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect c1 failed");
        let c2 = headless(ADDR).await.expect("connect c2 failed");
        tokio::time::sleep(Duration::from_millis(400)).await;

        // c1 triggers which-key with `g`
        c1.send_keys("g").await.unwrap();

        let f1 = c1
            .wait_for(Duration::from_secs(3), |f| f.contains('\u{256D}'))
            .await
            .expect("c1 which-key popup did not appear");
        dump("c1 whichkey popup", &f1);
        assert!(f1.contains('\u{256D}'), "c1 has which-key popup");

        // c2 should NOT see the which-key popup
        tokio::time::sleep(Duration::from_millis(300)).await;
        let f2 = c2.capture("plain_text").await.unwrap();
        dump("c2 while c1 has whichkey", &f2);
        assert!(!f2.contains('\u{256D}'), "c2 must NOT see c1's which-key popup");
        eprintln!("[ok] Which-key popup isolated: c2 does NOT see c1's popup");

        c1.send_keys("<Esc>").await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        c1.stop().await;
        c2.stop().await;
        eprintln!("[scenario 3d] PASSED\n");
    }

    // ==================================================================
    // Scenario 3e: Which-key rapid open/dismiss cycling
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 3e] Which-key rapid open/dismiss cycling");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect failed");
        tokio::time::sleep(Duration::from_millis(300)).await;

        for i in 0..3 {
            // Open with `g`
            c1.send_keys("g").await.unwrap();
            let f = c1
                .wait_for(Duration::from_secs(3), |f| f.contains('\u{256D}'))
                .await
                .unwrap_or_else(|_| panic!("whichkey popup did not open on cycle {i}"));
            assert!(f.contains('\u{256D}'));

            // Dismiss with Escape
            c1.send_keys("<Esc>").await.unwrap();
            let f = c1
                .wait_for(Duration::from_secs(2), |f| !f.contains('\u{256D}'))
                .await
                .unwrap_or_else(|_| panic!("whichkey popup did not close on cycle {i}"));
            assert!(!f.contains('\u{256D}'));

            eprintln!("[ok] Which-key cycle {i} -- open/close ok");
        }

        c1.stop().await;
        eprintln!("[scenario 3e] PASSED\n");
    }

    // ==================================================================
    // Scenario 4: Two clients — cmdline on c1 must NOT appear on c2
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 4] Multi-client cmdline isolation");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect c1 failed");
        let c2 = headless(ADDR).await.expect("connect c2 failed");
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Baseline: neither has popup
        let f1 = c1.capture("plain_text").await.unwrap();
        let f2 = c2.capture("plain_text").await.unwrap();
        assert!(!f1.contains('\u{256D}'), "c1 no popup at start");
        assert!(!f2.contains('\u{256D}'), "c2 no popup at start");
        eprintln!("[ok] Both clients start clean");

        // Client 1 activates cmdline
        c1.send_keys(":").await.unwrap();
        let f1 = c1
            .wait_for(Duration::from_secs(3), |f| f.contains('\u{256D}'))
            .await
            .expect("c1 popup did not appear");
        dump("c1 after `:`", &f1);
        assert!(f1.contains('\u{256D}'), "c1 has popup");

        // Client 2 should NOT see the popup
        tokio::time::sleep(Duration::from_millis(300)).await;
        let f2 = c2.capture("plain_text").await.unwrap();
        dump("c2 while c1 has cmdline", &f2);
        let c2_has_cmdline_popup = f2
            .lines()
            .any(|l| l.contains('\u{2502}') && l.contains(':'));
        assert!(!c2_has_cmdline_popup, "c2 must NOT see c1's cmdline popup");
        eprintln!("[ok] Client 2 does NOT see client 1's cmdline");

        // Client 1 types in cmdline
        for ch in ['h', 'e', 'l', 'p'] {
            c1.send_keys(&ch.to_string()).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(200)).await;

        let f1 = c1.capture("plain_text").await.unwrap();
        dump("c1 after typing 'help'", &f1);
        assert!(f1.lines().any(|l| l.contains("help")), "c1 should show 'help' in popup");

        let f2 = c2.capture("plain_text").await.unwrap();
        let c2_sees_help = f2
            .lines()
            .any(|l| l.contains('\u{2502}') && l.contains("help"));
        assert!(!c2_sees_help, "c2 must NOT see 'help' from c1's cmdline");
        eprintln!("[ok] Cmdline text isolated per-client");

        c1.send_keys("<Esc>").await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        c1.stop().await;
        c2.stop().await;
        eprintln!("[scenario 4] PASSED\n");
    }

    // ==================================================================
    // Scenario 5: Rapid cmdline open/close cycling (5 rounds)
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 5] Rapid cmdline open/close cycling");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect failed");
        tokio::time::sleep(Duration::from_millis(300)).await;

        for i in 0..5 {
            c1.send_keys(":").await.unwrap();
            let f = c1
                .wait_for(Duration::from_secs(2), |f| f.contains('\u{256D}'))
                .await
                .unwrap_or_else(|_| panic!("popup did not open on cycle {i}"));
            assert!(f.contains('\u{256D}'));

            c1.send_keys("<Esc>").await.unwrap();
            let f = c1
                .wait_for(Duration::from_secs(2), |f| !f.contains('\u{256D}'))
                .await
                .unwrap_or_else(|_| panic!("popup did not close on cycle {i}"));
            assert!(!f.contains('\u{256D}'));

            eprintln!("[ok] Cycle {i} -- open/close ok");
        }

        c1.stop().await;
        eprintln!("[scenario 5] PASSED\n");
    }

    // ==================================================================
    // Scenario 6: Edit -> Cmdline -> Edit flow
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 6] Edit -> Cmdline -> Edit flow");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect failed");
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Capture baseline buffer content
        let baseline = c1.capture("plain_text").await.unwrap();
        let first_line = baseline.lines().next().unwrap_or("").trim().to_string();
        eprintln!("[info] Baseline buffer first line: '{first_line}'");

        // Insert unique marker text
        c1.send_keys("o[S6]<Esc>").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| f.contains("[S6]"))
            .await
            .expect("text not inserted");
        dump("after insert", &frame);
        eprintln!("[ok] Text inserted");

        // Open cmdline
        c1.send_keys(":").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| f.contains('\u{256D}'))
            .await
            .expect("cmdline did not open");
        dump("cmdline open", &frame);
        assert!(frame.contains("[S6]"), "Buffer text should still render behind popup");
        assert!(frame.contains('\u{256D}'), "Popup should be visible");
        eprintln!("[ok] Cmdline open -- buffer text still visible");

        // Close cmdline
        c1.send_keys("<Esc>").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| !f.contains('\u{256D}'))
            .await
            .expect("cmdline did not close");
        dump("after cmdline closed", &frame);
        assert!(frame.contains("[S6]"), "Buffer text preserved");
        assert!(!frame.contains('\u{256D}'), "Popup gone");
        eprintln!("[ok] Buffer intact after cmdline dismiss");

        // Continue editing — append on new line to avoid cursor position issues
        c1.send_keys("o[S6-OK]<Esc>").await.unwrap();
        let frame = c1
            .wait_for(Duration::from_secs(2), |f| f.contains("[S6-OK]"))
            .await
            .expect("appended text not visible");
        dump("after appending marker", &frame);
        assert!(frame.contains("[S6-OK]"));
        eprintln!("[ok] Editing continues normally after cmdline");

        c1.stop().await;
        eprintln!("[scenario 6] PASSED\n");
    }

    // ==================================================================
    // Scenario 7: Two clients both use cmdline simultaneously
    // ==================================================================
    eprintln!("======================================================================");
    eprintln!("[scenario 7] Two clients both open cmdline simultaneously");
    eprintln!("======================================================================");
    {
        let c1 = headless(ADDR).await.expect("connect c1 failed");
        let c2 = headless(ADDR).await.expect("connect c2 failed");
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Both open cmdline
        c1.send_keys(":").await.unwrap();
        c2.send_keys(":").await.unwrap();

        let f1 = c1
            .wait_for(Duration::from_secs(3), |f| f.contains('\u{256D}'))
            .await
            .expect("c1 popup did not appear");
        let f2 = c2
            .wait_for(Duration::from_secs(3), |f| f.contains('\u{256D}'))
            .await
            .expect("c2 popup did not appear");

        dump("c1 cmdline", &f1);
        dump("c2 cmdline", &f2);

        // Client 1 types "abc", Client 2 types "xyz"
        for ch in ['a', 'b', 'c'] {
            c1.send_keys(&ch.to_string()).await.unwrap();
        }
        for ch in ['x', 'y', 'z'] {
            c2.send_keys(&ch.to_string()).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(300)).await;

        let f1 = c1.capture("plain_text").await.unwrap();
        let f2 = c2.capture("plain_text").await.unwrap();

        dump("c1 after typing 'abc'", &f1);
        dump("c2 after typing 'xyz'", &f2);

        let c1_row = f1
            .lines()
            .find(|l| l.contains('\u{2502}') && l.contains(':'));
        let c2_row = f2
            .lines()
            .find(|l| l.contains('\u{2502}') && l.contains(':'));

        if let Some(row) = c1_row {
            assert!(row.contains("abc"), "c1 should see 'abc', got: {row}");
            assert!(!row.contains("xyz"), "c1 must NOT see 'xyz'");
            eprintln!("[ok] c1 sees only its own input: {row}");
        }

        if let Some(row) = c2_row {
            assert!(row.contains("xyz"), "c2 should see 'xyz', got: {row}");
            assert!(!row.contains("abc"), "c2 must NOT see 'abc'");
            eprintln!("[ok] c2 sees only its own input: {row}");
        }

        c1.send_keys("<Esc>").await.unwrap();
        c2.send_keys("<Esc>").await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        c1.stop().await;
        c2.stop().await;
        eprintln!("[scenario 7] PASSED\n");
    }

    eprintln!("======================================================================");
    eprintln!("ALL SCENARIOS COMPLETE");
    eprintln!("======================================================================");
}
