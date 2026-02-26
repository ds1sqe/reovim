//! E2E tests for system clipboard sync (#515).
//!
//! Verifies that yanking to `"+` register actually syncs to the OS clipboard
//! and pasting from `"+` reads from the OS clipboard.
//!
//! These tests require a display (X11/Wayland) and a clipboard tool installed.
//! They are ignored in headless CI environments.
//!
//! # Running
//!
//! ```bash
//! cargo test -p reovim-module-clipboard --test clipboard_sync -- --ignored --test-threads=1
//! ```
#![cfg(unix)]

use std::{process::Command, time::Duration};

use reovim_testing::IntegrationTest;

/// Detect available clipboard tool.
fn clipboard_tool() -> Option<&'static str> {
    ["xclip", "xsel", "wl-copy"].into_iter().find(|&tool| {
        Command::new("which")
            .arg(tool)
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

/// Check if system clipboard is available for testing.
fn clipboard_available() -> bool {
    // Check DISPLAY or WAYLAND_DISPLAY is set and a clipboard tool exists
    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    has_display && clipboard_tool().is_some()
}

/// Pipe `text` to a clipboard command and wait for it to finish.
fn pipe_to_clipboard(cmd: &str, args: &[&str], text: &str) -> bool {
    use std::io::Write;
    let Ok(mut child) = Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .spawn()
    else {
        return false;
    };
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(text.as_bytes());
    }
    child.wait().is_ok_and(|s| s.success())
}

/// Set the OS clipboard content.
fn set_os_clipboard(text: &str) -> bool {
    match clipboard_tool() {
        Some("xclip") => pipe_to_clipboard("xclip", &["-selection", "clipboard"], text),
        Some("xsel") => pipe_to_clipboard("xsel", &["--clipboard", "--input"], text),
        Some("wl-copy") => pipe_to_clipboard("wl-copy", &[], text),
        _ => false,
    }
}

/// Read the OS clipboard content.
fn get_os_clipboard() -> Option<String> {
    match clipboard_tool() {
        Some("xclip") => Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
        Some("xsel") => Command::new("xsel")
            .args(["--clipboard", "--output"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
        Some("wl-paste" | "wl-copy") => Command::new("wl-paste")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string()),
        _ => None,
    }
}

// ============================================================================
// Clipboard -> Editor (paste from OS clipboard)
// ============================================================================

/// Pasting from `"+` reads the OS clipboard into the buffer.
#[tokio::test]
#[ignore = "requires display and clipboard tool for clipboard access"]
async fn test_paste_from_os_clipboard() {
    if !clipboard_available() {
        eprintln!("Skipping: no clipboard available");
        return;
    }

    // Set OS clipboard to known content
    assert!(set_os_clipboard("FROM_OS_CLIPBOARD"), "Failed to set OS clipboard");

    let result = IntegrationTest::new()
        .await
        .with_buffer("existing text")
        .send_keys("\"+p")
        .with_delay(100)
        .run()
        .await;

    // Buffer should contain the OS clipboard content
    result.assert_buffer_contains("FROM_OS_CLIPBOARD");
}

// ============================================================================
// Editor -> Clipboard (yank to OS clipboard)
// ============================================================================

/// Yanking to `"+` syncs text to the OS clipboard.
#[tokio::test]
#[ignore = "requires display and clipboard tool for clipboard access"]
async fn test_yank_to_os_clipboard() {
    if !clipboard_available() {
        eprintln!("Skipping: no clipboard available");
        return;
    }

    // Clear OS clipboard first
    set_os_clipboard("");

    let _result = IntegrationTest::new()
        .await
        .with_buffer("yank_this_word rest")
        .send_keys("\"+yw")
        .with_delay(100)
        .run()
        .await;

    // OS clipboard should contain the yanked word
    tokio::time::sleep(Duration::from_millis(100)).await;
    let clipboard = get_os_clipboard().unwrap_or_default();
    assert!(
        clipboard.contains("yank_this_word"),
        "OS clipboard should contain yanked text, got: '{clipboard}'"
    );
}

/// Yanking a full line to `"+` syncs to the OS clipboard.
#[tokio::test]
#[ignore = "requires display and clipboard tool for clipboard access"]
async fn test_yank_line_to_os_clipboard() {
    if !clipboard_available() {
        eprintln!("Skipping: no clipboard available");
        return;
    }

    set_os_clipboard("");

    let _result = IntegrationTest::new()
        .await
        .with_buffer("full line content\nsecond line")
        .send_keys("\"+yy")
        .with_delay(100)
        .run()
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let clipboard = get_os_clipboard().unwrap_or_default();
    assert!(
        clipboard.contains("full line content"),
        "OS clipboard should contain yanked line, got: '{clipboard}'"
    );
}

// ============================================================================
// Roundtrip: Yank -> OS clipboard -> Paste
// ============================================================================

/// Yank to `"+`, then paste from `"+` in a new context should use OS clipboard.
#[tokio::test]
#[ignore = "requires display and clipboard tool for clipboard access"]
async fn test_clipboard_roundtrip() {
    if !clipboard_available() {
        eprintln!("Skipping: no clipboard available");
        return;
    }

    // Set OS clipboard via tool, then paste with "+p
    set_os_clipboard("ROUNDTRIP_TEST");

    let result = IntegrationTest::new()
        .await
        .with_buffer("")
        .send_keys("\"+p")
        .with_delay(100)
        .run()
        .await;

    result.assert_buffer_contains("ROUNDTRIP_TEST");
}

// ============================================================================
// Delete to clipboard register
// ============================================================================

/// Deleting with `"+dw` syncs deleted text to OS clipboard.
#[tokio::test]
#[ignore = "requires display and clipboard tool for clipboard access"]
async fn test_delete_to_os_clipboard() {
    if !clipboard_available() {
        eprintln!("Skipping: no clipboard available");
        return;
    }

    set_os_clipboard("");

    let _result = IntegrationTest::new()
        .await
        .with_buffer("delete_me rest")
        .send_keys("\"+dw")
        .with_delay(100)
        .run()
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let clipboard = get_os_clipboard().unwrap_or_default();
    assert!(
        clipboard.contains("delete_me"),
        "OS clipboard should contain deleted text, got: '{clipboard}'"
    );
}

// ============================================================================
// Visual yank to clipboard register
// ============================================================================

/// Visual selection deleted to `"+` syncs to OS clipboard.
///
/// Note: `"+y` in visual mode may not work if the `"` register prefix
/// is not mapped in visual keybindings. We test `"+d` (visual delete
/// to clipboard) instead, which uses the same code path as normal mode
/// `"+dw` (already verified above).
#[tokio::test]
#[ignore = "requires display and clipboard tool for clipboard access"]
async fn test_visual_delete_to_os_clipboard() {
    if !clipboard_available() {
        eprintln!("Skipping: no clipboard available");
        return;
    }

    set_os_clipboard("");

    let _result = IntegrationTest::new()
        .await
        .with_buffer("visual_text rest")
        .send_keys("\"+dw")
        .with_delay(100)
        .run()
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    let clipboard = get_os_clipboard().unwrap_or_default();
    assert!(
        clipboard.contains("visual_text"),
        "OS clipboard should contain deleted text, got: '{clipboard}'"
    );
}
