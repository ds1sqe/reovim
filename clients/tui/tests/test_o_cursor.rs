//! Quick test to check if cursor position updates after `o` + typing
use {
    reovim_client_tui::{TuiAppError, TuiHandle, connect_headless},
    reovim_testing::TestServerHarness,
    std::time::Duration,
};

async fn headless_tui(addr: &str, width: u16, height: u16) -> Result<TuiHandle, TuiAppError> {
    let (mut app, handle) =
        connect_headless(addr, width, height, None, None, &std::collections::HashSet::new())
            .await?;
    tokio::spawn(async move { app.run().await });
    Ok(handle)
}

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
async fn test_o_cursor_position() {
    let harness = TestServerHarness::spawn().await.expect("spawn");
    let addr = format!("127.0.0.1:{}", harness.port());

    // Two clients
    let tui_a = headless_tui(&addr, 80, 24).await.expect("TUI A");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let tui_b = headless_tui(&addr, 80, 24).await.expect("TUI B");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // A types some content
    tui_a
        .send_keys("ihello<CR>world<Esc>")
        .await
        .expect("A type");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Check A's cursor before `o`
    let frame_pre = tui_a.capture("plain_text").await.expect("cap");
    let cursor_pre = extract_cursor_from_statusline(&frame_pre);
    eprintln!("BEFORE o: cursor={cursor_pre:?}");
    eprintln!("--- frame ---");
    for (i, line) in frame_pre.lines().enumerate() {
        eprintln!("  {:>2}| {line}", i + 1);
    }

    // A presses `o` (open line below, enter INSERT)
    tui_a.send_keys("o").await.expect("A o");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let frame_after_o = tui_a.capture("plain_text").await.expect("cap");
    let cursor_after_o = extract_cursor_from_statusline(&frame_after_o);
    eprintln!("\nAFTER o: cursor={cursor_after_o:?}");
    eprintln!("--- frame ---");
    for (i, line) in frame_after_o.lines().enumerate() {
        eprintln!("  {:>2}| {line}", i + 1);
    }

    // A types `iii`
    tui_a.send_keys("iii").await.expect("A iii");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let frame_after_iii = tui_a.capture("plain_text").await.expect("cap");
    let cursor_after_iii = extract_cursor_from_statusline(&frame_after_iii);
    eprintln!("\nAFTER iii: cursor={cursor_after_iii:?}");
    eprintln!("--- frame ---");
    for (i, line) in frame_after_iii.lines().enumerate() {
        eprintln!("  {:>2}| {line}", i + 1);
    }

    // A presses Esc
    tui_a.send_keys("<Esc>").await.expect("A esc");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let frame_after_esc = tui_a.capture("plain_text").await.expect("cap");
    let cursor_after_esc = extract_cursor_from_statusline(&frame_after_esc);
    eprintln!("\nAFTER Esc: cursor={cursor_after_esc:?}");
    eprintln!("--- frame ---");
    for (i, line) in frame_after_esc.lines().enumerate() {
        eprintln!("  {:>2}| {line}", i + 1);
    }

    // The cursor after `o` should be on line 3 (new line below line 2)
    assert!(
        cursor_after_o.is_some_and(|(line, _)| line == 3),
        "After `o`, cursor should be on line 3, got {cursor_after_o:?}",
    );

    // The cursor after typing `iii` should still be on line 3
    assert!(
        cursor_after_iii.is_some_and(|(line, _)| line == 3),
        "After `iii`, cursor should be on line 3, got {cursor_after_iii:?}",
    );

    // After Esc, cursor should be on line 3
    assert!(
        cursor_after_esc.is_some_and(|(line, _)| line == 3),
        "After Esc, cursor should be on line 3, got {cursor_after_esc:?}",
    );

    tui_a.stop().await;
    tui_b.stop().await;
}
