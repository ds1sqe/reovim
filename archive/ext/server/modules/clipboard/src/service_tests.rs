use {super::*, reovim_subsys_clipboard::MockClipboardProvider};

// ============================================================================
// Mock-based tests (run in CI without display)
// ============================================================================

#[test]
fn mock_clipboard_roundtrip() {
    let provider = MockClipboardProvider::new();
    let test_text = "reovim clipboard test";
    provider.copy_to_clipboard(test_text).unwrap();
    let pasted = provider.paste_from_clipboard().unwrap();
    assert_eq!(pasted, Some(test_text.to_string()));
}

#[test]
fn mock_clipboard_selection_roundtrip() {
    let provider = MockClipboardProvider::new();
    provider.copy_to_selection("selection text").unwrap();
    let pasted = provider.paste_from_selection().unwrap();
    assert_eq!(pasted, Some("selection text".to_string()));
}

#[test]
fn mock_clipboard_empty_paste_returns_none() {
    let provider = MockClipboardProvider::new();
    assert!(provider.paste_from_clipboard().unwrap().is_none());
    assert!(provider.paste_from_selection().unwrap().is_none());
}

#[test]
fn mock_clipboard_overwrite() {
    let provider = MockClipboardProvider::new();
    provider.copy_to_clipboard("first").unwrap();
    provider.copy_to_clipboard("second").unwrap();
    assert_eq!(provider.paste_from_clipboard().unwrap(), Some("second".to_string()));
}

#[test]
fn mock_clipboard_register_and_use() {
    use reovim_subsys_clipboard::{ClipboardKey, ClipboardProviderRegistry};

    let registry = ClipboardProviderRegistry::new();
    let mock = std::sync::Arc::new(MockClipboardProvider::new());
    registry.register(ClipboardKey::Default, mock);

    let provider = registry.get(&ClipboardKey::Default).unwrap();
    assert!(provider.clipboard_available());
    provider.copy_to_clipboard("via registry").unwrap();
    assert_eq!(provider.paste_from_clipboard().unwrap(), Some("via registry".to_string()));
}

// ============================================================================
// Real system clipboard test (display-dependent, ignored in CI)
// ============================================================================

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
