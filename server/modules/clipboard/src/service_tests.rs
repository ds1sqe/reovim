use super::*;

// Note: System clipboard tests are skipped in CI as they require a display
// Run locally with: cargo test -p reovim-module-clipboard -- --ignored
#[test]
#[ignore = "requires display for clipboard access (#576)"]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_system_clipboard() {
    let service = ClipboardService::new();

    if service.clipboard_available() {
        let test_text = "reovim clipboard test";
        service.copy_to_clipboard(test_text).unwrap();
        let pasted = service.paste_from_clipboard().unwrap();
        assert_eq!(pasted, Some(test_text.to_string()));
    }
}

#[test]
fn test_default_creates_same_as_new() {
    // Both start with no state - just verify they don't panic
    let _from_new = ClipboardService::new();
    let _from_default = ClipboardService::default();
}

#[test]
fn test_debug_impl() {
    let service = ClipboardService::new();
    let debug = format!("{service:?}");
    assert!(debug.contains("ClipboardService"));
    assert!(debug.contains("clipboard_available"));
}

// System clipboard tests - exercise the delegation paths.
// These may fail in CI without a display, but we test the code paths
// that don't depend on clipboard availability.

#[test]
fn test_selection_available_delegates_to_clipboard() {
    let service = ClipboardService::new();
    assert_eq!(service.selection_available(), service.clipboard_available());
}

#[test]
fn test_clipboard_available_consistent() {
    let service = ClipboardService::new();
    let first = service.clipboard_available();
    let second = service.clipboard_available();
    assert_eq!(first, second);
}

#[test]
fn test_copy_to_selection_when_no_display() {
    let service = ClipboardService::new();
    let _ = service.copy_to_selection("test");
}

#[test]
fn test_paste_from_selection_when_no_display() {
    let service = ClipboardService::new();
    let _ = service.paste_from_selection();
}

#[test]
fn test_copy_to_clipboard_when_no_display() {
    let service = ClipboardService::new();
    let _ = service.copy_to_clipboard("test");
}

#[test]
fn test_paste_from_clipboard_when_no_display() {
    let service = ClipboardService::new();
    let _ = service.paste_from_clipboard();
}

#[test]
fn test_clipboard_available_called_multiple_times() {
    let service = ClipboardService::new();
    for _ in 0..5 {
        let _ = service.clipboard_available();
    }
}

#[test]
fn test_selection_available_matches_clipboard() {
    let service = ClipboardService::new();
    let clip = service.clipboard_available();
    let sel = service.selection_available();
    assert_eq!(clip, sel);
}

#[test]
fn test_any_clipboard_available() {
    let service = ClipboardService::new();
    // any_clipboard_available = clipboard || selection
    // Both delegate to same underlying, so they agree
    let any = service.any_clipboard_available();
    assert_eq!(any, service.clipboard_available());
}
