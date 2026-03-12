use {
    super::*,
    reovim_driver_display::{FrameBuffer, render_backend::TuiExtension},
};

// ========================================================================
// Initialization and state
// ========================================================================

#[test]
fn test_new_is_active() {
    let ext = LandingExtension::new();
    assert!(ext.is_active());
}

#[test]
fn test_default_is_active() {
    let ext = LandingExtension::default();
    assert!(ext.is_active());
}

#[test]
fn test_kind_returns_landing() {
    let ext = LandingExtension::new();
    assert_eq!(ext.kind(), "landing");
}

#[test]
fn test_trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(LandingExtension::new());
    assert_eq!(ext.kind(), "landing");
    assert!(ext.is_active());
}

// ========================================================================
// Dismissal behavior
// ========================================================================

#[test]
fn test_dismiss_on_cursor_update() {
    let mut ext = LandingExtension::new();
    assert!(ext.is_active());
    ext.on_cursor_update(0, 0, 0);
    assert!(!ext.is_active());
}

#[test]
fn test_dismiss_on_mode_change() {
    let mut ext = LandingExtension::new();
    assert!(ext.is_active());
    ext.on_mode_change("insert", true);
    assert!(!ext.is_active());
}

#[test]
fn test_dismiss_on_buffer_update() {
    let mut ext = LandingExtension::new();
    assert!(ext.is_active());
    ext.on_buffer_update(0, &["hello".to_string()]);
    assert!(!ext.is_active());
}

#[test]
fn test_dismiss_is_permanent() {
    let mut ext = LandingExtension::new();
    ext.on_cursor_update(0, 0, 0);
    assert!(!ext.is_active());
    // Cannot re-activate.
    assert!(ext.dismissed);
}

// ========================================================================
// No-op methods
// ========================================================================

#[test]
fn test_apply_notification_noop() {
    let mut ext = LandingExtension::new();
    ext.apply_notification("anything");
    assert!(ext.is_active());
}

#[test]
fn test_apply_notification_empty() {
    let mut ext = LandingExtension::new();
    ext.apply_notification("");
    assert!(ext.is_active());
}

#[test]
fn test_tick_returns_false() {
    let mut ext = LandingExtension::new();
    assert!(!ext.tick());
}

// ========================================================================
// Rendering
// ========================================================================

#[test]
fn test_render_on_normal_terminal() {
    let ext = LandingExtension::new();
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Verify that something was rendered (not all spaces).
    let mut has_content = false;
    for y in 0..24 {
        for x in 0..80 {
            if let Some(cell) = fb.get(x, y)
                && cell.char != ' '
            {
                has_content = true;
                break;
            }
        }
        if has_content {
            break;
        }
    }
    assert!(has_content, "Landing screen should render content");
}

#[test]
fn test_render_on_small_terminal() {
    let ext = LandingExtension::new();
    let mut fb = FrameBuffer::new(20, 5);
    ext.render(&mut fb);

    // Should gracefully skip rendering — all cells remain space.
    for y in 0..5 {
        for x in 0..20 {
            if let Some(cell) = fb.get(x, y) {
                assert_eq!(
                    cell.char, ' ',
                    "Small terminal should not render landing: ({x}, {y}) = '{}'",
                    cell.char,
                );
            }
        }
    }
}

#[test]
fn test_render_skipped_when_inactive() {
    let mut ext = LandingExtension::new();
    ext.on_cursor_update(0, 0, 0);

    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);

    // Should not render anything after dismissal.
    for y in 0..24 {
        for x in 0..80 {
            if let Some(cell) = fb.get(x, y) {
                assert_eq!(
                    cell.char, ' ',
                    "Dismissed extension should not render: ({x}, {y}) = '{}'",
                    cell.char,
                );
            }
        }
    }
}

#[test]
fn test_render_content_centered() {
    let ext = LandingExtension::new();
    let mut fb = FrameBuffer::new(120, 40);
    ext.render(&mut fb);

    // The box should be centered. BOX_WIDTH = 42, so box_x = (120 - 42) / 2 = 39.
    // Check that the top-left corner border character is at (39, box_y).
    let box_x: u16 = (120 - BOX_WIDTH) / 2;
    let box_y: u16 = (40 - 17) / 2; // box_height = 17

    // Top-left corner should be ╭ (U+256D).
    let cell = fb.get(box_x, box_y).expect("cell should exist");
    assert_eq!(cell.char, '\u{256D}', "Expected top-left border corner");

    // Top-right corner should be ╮ (U+256E).
    let cell = fb
        .get(box_x + BOX_WIDTH - 1, box_y)
        .expect("cell should exist");
    assert_eq!(cell.char, '\u{256E}', "Expected top-right border corner");
}

#[test]
fn test_render_large_terminal() {
    let ext = LandingExtension::new();
    let mut fb = FrameBuffer::new(200, 60);
    ext.render(&mut fb);

    // Just verify it doesn't panic and renders something.
    let mut has_content = false;
    for y in 0..60 {
        for x in 0..200_u16 {
            if let Some(cell) = fb.get(x, y)
                && cell.char != ' '
            {
                has_content = true;
                break;
            }
        }
        if has_content {
            break;
        }
    }
    assert!(has_content);
}
