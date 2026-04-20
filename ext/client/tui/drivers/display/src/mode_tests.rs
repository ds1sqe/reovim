use super::*;

#[test]
fn test_cursor_style_default() {
    let style = CursorStyle::default();
    assert_eq!(style, CursorStyle::Block);
}

#[test]
fn test_cursor_style_is_visible() {
    assert!(CursorStyle::Block.is_visible());
    assert!(CursorStyle::Bar.is_visible());
    assert!(CursorStyle::Underline.is_visible());
    assert!(!CursorStyle::Hidden.is_visible());
}

#[test]
fn test_cursor_style_name() {
    assert_eq!(CursorStyle::Block.name(), "block");
    assert_eq!(CursorStyle::Bar.name(), "bar");
    assert_eq!(CursorStyle::Underline.name(), "underline");
    assert_eq!(CursorStyle::Hidden.name(), "hidden");
}

#[test]
fn test_mode_display_trait_object_safety() {
    // Verify ModeDisplay trait is object-safe
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn _accepts_ref(_: &dyn ModeDisplay) {}
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn _accepts_box(_: Box<dyn ModeDisplay>) {}
}

// Test implementation
struct TestMode {
    cursor: CursorStyle,
    status: &'static str,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ModeDisplay for TestMode {
    fn cursor_style(&self) -> CursorStyle {
        self.cursor
    }

    fn status_text(&self) -> &'static str {
        self.status
    }
}

#[test]
fn test_mode_display_implementation() {
    let mode = TestMode {
        cursor: CursorStyle::Bar,
        status: "TEST",
    };
    assert_eq!(mode.cursor_style(), CursorStyle::Bar);
    assert_eq!(mode.status_text(), "TEST");
}

#[test]
fn test_mode_display_default_status() {
    struct MinimalMode;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ModeDisplay for MinimalMode {
        fn cursor_style(&self) -> CursorStyle {
            CursorStyle::Block
        }
    }

    let mode = MinimalMode;
    assert_eq!(mode.status_text(), "");
}
