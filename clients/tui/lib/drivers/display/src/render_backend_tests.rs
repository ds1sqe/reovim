use super::*;

// =========================================================================
// RenderBackend for FrameBuffer tests
// =========================================================================

#[test]
fn test_framebuffer_set_cell() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default();
    fb.set_cell(0, 0, 'H', &style);
    fb.set_cell(1, 0, 'i', &style);
    assert_eq!(fb.get(0, 0).unwrap().char, 'H');
    assert_eq!(fb.get(1, 0).unwrap().char, 'i');
}

#[test]
fn test_framebuffer_write_str() {
    let mut fb = FrameBuffer::new(20, 5);
    let style = Style::default();
    let cols = RenderBackend::write_str(&mut fb, 0, 0, "Hello", &style);
    assert_eq!(cols, 5);
    assert_eq!(fb.get(0, 0).unwrap().char, 'H');
    assert_eq!(fb.get(4, 0).unwrap().char, 'o');
}

#[test]
fn test_framebuffer_size() {
    let fb = FrameBuffer::new(80, 24);
    assert_eq!(RenderBackend::size(&fb), (80, 24));
}

#[test]
fn test_framebuffer_clear() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default();
    fb.set_cell(0, 0, 'X', &style);
    RenderBackend::clear(&mut fb);
    assert_eq!(fb.get(0, 0).unwrap().char, ' ');
}

#[test]
fn test_framebuffer_apply_style() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default();
    fb.set_cell(0, 0, 'A', &style);
    let highlight = Style::default().bg(Color::Yellow);
    RenderBackend::apply_style(&mut fb, 0, 0, &highlight);
    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.char, 'A'); // Character preserved
    assert_eq!(cell.style.bg, Some(Color::Yellow));
}

#[test]
fn test_framebuffer_overlay_bg() {
    let mut fb = FrameBuffer::new(10, 5);
    let style = Style::default().fg(Color::White);
    fb.set_cell(0, 0, 'B', &style);
    RenderBackend::overlay_bg(&mut fb, 0, 0, Color::Blue);
    let cell = fb.get(0, 0).unwrap();
    assert_eq!(cell.char, 'B');
    assert_eq!(cell.style.bg, Some(Color::Blue));
}

#[test]
fn test_framebuffer_fill_horizontal() {
    let mut fb = FrameBuffer::new(20, 5);
    let style = Style::default();
    RenderBackend::fill_horizontal(&mut fb, 2, 1, 5, '-', &style);
    assert_eq!(fb.get(2, 1).unwrap().char, '-');
    assert_eq!(fb.get(6, 1).unwrap().char, '-');
    assert_eq!(fb.get(7, 1).unwrap().char, ' ');
}

#[test]
fn test_framebuffer_fill_vertical() {
    let mut fb = FrameBuffer::new(10, 10);
    let style = Style::default();
    RenderBackend::fill_vertical(&mut fb, 3, 1, 4, '|', &style);
    assert_eq!(fb.get(3, 1).unwrap().char, '|');
    assert_eq!(fb.get(3, 4).unwrap().char, '|');
    assert_eq!(fb.get(3, 5).unwrap().char, ' ');
}

#[test]
fn test_framebuffer_fill_region() {
    let mut fb = FrameBuffer::new(20, 10);
    let style = Style::default();
    RenderBackend::fill_region(&mut fb, 1, 1, 3, 2, '#', &style);
    assert_eq!(fb.get(1, 1).unwrap().char, '#');
    assert_eq!(fb.get(3, 2).unwrap().char, '#');
    assert_eq!(fb.get(4, 1).unwrap().char, ' ');
}

// =========================================================================
// TuiExtension trait tests
// =========================================================================

use std::sync::atomic::{AtomicBool, Ordering};

struct MockExtension {
    active: bool,
    rendered: AtomicBool,
}

impl TuiExtension for MockExtension {
    fn kind(&self) -> &'static str {
        "mock"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn apply_notification(&mut self, _data: &str) {
        self.active = true;
    }

    fn render(&self, backend: &mut dyn RenderBackend) {
        self.rendered.store(true, Ordering::Relaxed);
        backend.set_cell(0, 0, 'M', &Style::default());
    }
}

#[test]
fn test_extension_trait_dispatch() {
    let mut ext = MockExtension {
        active: false,
        rendered: AtomicBool::new(false),
    };
    assert_eq!(ext.kind(), "mock");
    assert!(!ext.is_active());

    ext.apply_notification(r#"{"active":true}"#);
    assert!(ext.is_active());

    let mut fb = FrameBuffer::new(10, 5);
    ext.render(&mut fb);
    assert!(ext.rendered.load(Ordering::Relaxed));
    assert_eq!(fb.get(0, 0).unwrap().char, 'M');
}

#[test]
fn test_extension_default_cursor_position() {
    let ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    // Default cursor_position returns None
    assert!(ext.cursor_position(80, 24).is_none());
}

#[test]
fn test_extension_default_tick() {
    let mut ext = MockExtension {
        active: false,
        rendered: AtomicBool::new(false),
    };
    // Default tick returns false (no-op)
    assert!(!ext.tick());
}

#[test]
fn test_extension_default_dependencies_empty() {
    let ext = MockExtension {
        active: false,
        rendered: AtomicBool::new(false),
    };
    assert!(ext.dependencies().is_empty());
}

#[test]
fn test_extension_default_init_noop() {
    let mut ext = MockExtension {
        active: false,
        rendered: AtomicBool::new(false),
    };
    // Should not panic
    ext.init();
}

#[test]
fn test_extension_default_exit_noop() {
    let mut ext = MockExtension {
        active: false,
        rendered: AtomicBool::new(false),
    };
    // Should not panic
    ext.exit();
}

#[test]
fn test_extension_trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    });
    assert_eq!(ext.kind(), "mock");
    assert!(ext.is_active());

    let mut fb = FrameBuffer::new(10, 5);
    ext.render(&mut fb);
    assert_eq!(fb.get(0, 0).unwrap().char, 'M');
}

// =========================================================================
// ViewportContext tests
// =========================================================================

#[test]
fn test_viewport_context_construction() {
    let ctx = ViewportContext {
        scroll_top: 10,
        content_x: 4,
        content_height: 20,
        buffer_id: None,
    };
    assert_eq!(ctx.scroll_top, 10);
    assert_eq!(ctx.content_x, 4);
    assert_eq!(ctx.content_height, 20);
}

#[test]
fn test_viewport_context_clone() {
    let ctx = ViewportContext {
        scroll_top: 5,
        content_x: 3,
        content_height: 15,
        buffer_id: None,
    };
    let cloned = ctx.clone();
    assert_eq!(cloned.scroll_top, ctx.scroll_top);
    assert_eq!(cloned.content_x, ctx.content_x);
    assert_eq!(cloned.content_height, ctx.content_height);
}

#[test]
fn test_viewport_context_debug() {
    let ctx = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 24,
        buffer_id: None,
    };
    let debug = format!("{ctx:?}");
    assert!(debug.contains("scroll_top"));
    assert!(debug.contains("content_x"));
    assert!(debug.contains("content_height"));
}

// =========================================================================
// render_with_viewport default tests
// =========================================================================

#[test]
fn test_render_with_viewport_default_delegates_to_render() {
    let ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 4,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(10, 5);
    ext.render_with_viewport(&mut fb, &viewport);
    // Default impl delegates to render(), which sets cell (0,0) to 'M'
    assert!(ext.rendered.load(Ordering::Relaxed));
    assert_eq!(fb.get(0, 0).unwrap().char, 'M');
}

#[test]
fn test_render_with_viewport_trait_object() {
    let ext: Box<dyn TuiExtension> = Box::new(MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    });
    let viewport = ViewportContext {
        scroll_top: 100,
        content_x: 6,
        content_height: 30,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(10, 5);
    ext.render_with_viewport(&mut fb, &viewport);
    assert_eq!(fb.get(0, 0).unwrap().char, 'M');
}

/// Extension that uses viewport context for buffer-position rendering.
struct ViewportAwareExtension;

impl TuiExtension for ViewportAwareExtension {
    fn kind(&self) -> &'static str {
        "viewport-aware"
    }

    fn is_active(&self) -> bool {
        true
    }

    fn apply_notification(&mut self, _data: &str) {}

    fn render(&self, _backend: &mut dyn RenderBackend) {
        // Intentionally empty — this extension uses render_with_viewport
    }

    fn render_with_viewport(&self, backend: &mut dyn RenderBackend, viewport: &ViewportContext) {
        // Render a marker at buffer position (0, 0) mapped to screen
        let screen_x = viewport.content_x;
        let screen_y = 0u16;
        if viewport.scroll_top == 0 {
            backend.set_cell(screen_x, screen_y, 'V', &Style::default());
        }
    }
}

#[test]
fn test_viewport_aware_extension_uses_viewport() {
    let ext = ViewportAwareExtension;
    let viewport = ViewportContext {
        scroll_top: 0,
        content_x: 5,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(20, 10);
    ext.render_with_viewport(&mut fb, &viewport);
    // Should render 'V' at (content_x, 0) = (5, 0)
    assert_eq!(fb.get(5, 0).unwrap().char, 'V');
}

#[test]
fn test_extension_default_fold_hidden_lines() {
    let ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    assert!(ext.fold_hidden_lines().is_empty());
}

#[test]
fn test_extension_default_classify_token() {
    let ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    assert!(ext.classify_token("markup.heading.1").is_none());
    assert!(ext.classify_token("keyword").is_none());
}

#[test]
fn test_extension_default_virtual_lines_empty() {
    let ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    assert!(ext.virtual_lines().is_empty());
}

#[test]
fn test_extension_default_transform_line_none() {
    let ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    assert!(ext.transform_line(1, 0, "hello").is_none());
}

#[test]
fn test_extension_default_map_cursor_column_none() {
    let ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    assert!(ext.map_cursor_column(1, 0, 5).is_none());
}

#[test]
fn test_extension_default_on_buffer_update_noop() {
    let mut ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    // Should not panic — just a no-op
    ext.on_buffer_update(1, &["hello".to_string()]);
}

#[test]
fn test_extension_default_on_cursor_update_noop() {
    let mut ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    ext.on_cursor_update(1, 0, 5);
}

#[test]
fn test_extension_default_on_mode_change_noop() {
    let mut ext = MockExtension {
        active: true,
        rendered: AtomicBool::new(false),
    };
    ext.on_mode_change("NORMAL", false);
}

#[test]
fn test_render_behavior_debug() {
    let highlight = RenderBehavior::Highlight;
    let conceal = RenderBehavior::Conceal {
        replacement: Cow::Borrowed("icon"),
    };
    let bg = RenderBehavior::Background;
    let hide = RenderBehavior::Hide;
    let full = RenderBehavior::FullWidthLine { ch: '─' };
    assert!(format!("{highlight:?}").contains("Highlight"));
    assert!(format!("{conceal:?}").contains("icon"));
    assert!(format!("{bg:?}").contains("Background"));
    assert!(format!("{hide:?}").contains("Hide"));
    assert!(format!("{full:?}").contains("FullWidthLine"));
}

#[test]
fn test_render_behavior_clone() {
    let original = RenderBehavior::Conceal {
        replacement: Cow::Borrowed("test"),
    };
    let _cloned = original.clone();
    if let RenderBehavior::Conceal { replacement } = original {
        assert_eq!(replacement.as_ref(), "test");
    } else {
        panic!("Clone should preserve variant");
    }
}

#[test]
fn test_virtual_line_position_eq() {
    assert_eq!(VirtualLinePosition::Before, VirtualLinePosition::Before);
    assert_eq!(VirtualLinePosition::After, VirtualLinePosition::After);
    assert_ne!(VirtualLinePosition::Before, VirtualLinePosition::After);
}

#[test]
fn test_virtual_line_construction() {
    let vl = VirtualLine {
        buffer_line: 5,
        position: VirtualLinePosition::Before,
        content: "┌───┐".to_string(),
        style: Style::default(),
    };
    assert_eq!(vl.buffer_line, 5);
    assert_eq!(vl.position, VirtualLinePosition::Before);
    assert_eq!(vl.content, "┌───┐");
}

#[test]
fn test_transformed_line_construction() {
    let tl = TransformedLine {
        text: "│ hello │".to_string(),
        styles: vec![None; 9],
    };
    assert_eq!(tl.text, "│ hello │");
    assert_eq!(tl.styles.len(), 9);
}

#[test]
fn test_viewport_aware_extension_scrolled() {
    let ext = ViewportAwareExtension;
    let viewport = ViewportContext {
        scroll_top: 10, // scrolled past line 0
        content_x: 5,
        content_height: 20,
        buffer_id: None,
    };
    let mut fb = FrameBuffer::new(20, 10);
    ext.render_with_viewport(&mut fb, &viewport);
    // Line 0 is above viewport, nothing rendered
    assert_eq!(fb.get(5, 0).unwrap().char, ' ');
}

#[test]
fn test_extension_default_server_kinds_returns_own_kind() {
    let ext = MockExtension {
        active: false,
        rendered: AtomicBool::new(false),
    };
    assert_eq!(ext.server_kinds(), vec!["mock"]);
}
