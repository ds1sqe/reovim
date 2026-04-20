//! Round-trip and dispatch tests for the FFI boundary types (#723).

#![allow(unsafe_code)]

use std::{
    borrow::Cow,
    sync::{Mutex, atomic::Ordering},
};

use {
    super::*,
    crate::{
        AnnotationContext, BufferId, ColorDepth, ColumnWidth, GutterCell, InlineDecoration, Insets,
        Rect, RenderBehavior, RenderSurface, RenderingModel, Style, ThemeProvider, TransformedLine,
        VirtualLine, VirtualLinePosition,
        testing::{MockPlatformCapabilities, MockThemeProvider},
        types::{Attributes, Color},
    },
};

// =============================================================================
// Helpers
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
enum RenderOp {
    Write {
        x: u16,
        y: u16,
        text: String,
        style: Style,
    },
    ApplyStyle {
        x: u16,
        y: u16,
        style: Style,
    },
    OverlayBg {
        x: u16,
        y: u16,
        bg: Color,
    },
    Fill {
        rect: Rect,
        ch: char,
        style: Style,
    },
    Clear {
        rect: Rect,
    },
}

#[derive(Default)]
struct MockRenderSurface {
    ops: Vec<RenderOp>,
    size: (u16, u16),
}

impl MockRenderSurface {
    fn with_size(cols: u16, rows: u16) -> Self {
        Self {
            ops: Vec::new(),
            size: (cols, rows),
        }
    }
}

impl RenderSurface for MockRenderSurface {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        self.ops.push(RenderOp::Write {
            x,
            y,
            text: text.to_string(),
            style,
        });
        text.chars().count() as u16
    }

    fn apply_style(&mut self, x: u16, y: u16, style: Style) {
        self.ops.push(RenderOp::ApplyStyle { x, y, style });
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        self.ops.push(RenderOp::OverlayBg { x, y, bg });
    }

    fn fill(&mut self, rect: Rect, ch: char, style: Style) {
        self.ops.push(RenderOp::Fill { rect, ch, style });
    }

    fn clear(&mut self, rect: Rect) {
        self.ops.push(RenderOp::Clear { rect });
    }

    fn size(&self) -> (u16, u16) {
        self.size
    }
}

fn sample_style() -> Style {
    Style::new()
        .fg(Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        })
        .bg(Color::AnsiValue(42))
        .bold()
        .italic()
}

static TRANSFORMED_LINE_TEST_LOCK: Mutex<()> = Mutex::new(());

// =============================================================================
// T-1 Round-trip tests — FfiColor / FfiStyle
// =============================================================================

#[test]
fn color_roundtrip_named_variants() {
    let variants = [
        Color::Reset,
        Color::Black,
        Color::DarkRed,
        Color::DarkGreen,
        Color::DarkYellow,
        Color::DarkBlue,
        Color::DarkMagenta,
        Color::DarkCyan,
        Color::Grey,
        Color::DarkGrey,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::White,
    ];
    for (expected_tag, color) in variants.iter().enumerate() {
        let ffi = FfiColor::from_color(*color);
        assert_eq!(ffi.tag, expected_tag as u8);
        assert_eq!(ffi.into_color(), *color);
    }
}

#[test]
fn color_roundtrip_ansi_value() {
    for idx in [0u8, 1, 42, 128, 254, 255] {
        let ffi = FfiColor::from_color(Color::AnsiValue(idx));
        assert_eq!(ffi.tag, 17);
        assert_eq!(ffi.r, idx);
        assert_eq!(ffi.into_color(), Color::AnsiValue(idx));
    }
}

#[test]
fn color_roundtrip_rgb() {
    let cases = [(0, 0, 0), (255, 255, 255), (42, 84, 126), (200, 100, 50)];
    for (r, g, b) in cases {
        let ffi = FfiColor::from_color(Color::Rgb { r, g, b });
        assert_eq!(ffi.tag, 18);
        assert_eq!((ffi.r, ffi.g, ffi.b), (r, g, b));
        assert_eq!(ffi.into_color(), Color::Rgb { r, g, b });
    }
}

#[test]
fn color_option_roundtrip_none() {
    let ffi = FfiColor::from_option(None);
    assert_eq!(ffi.tag, 255);
    assert_eq!(ffi.into_option(), None);
}

#[test]
fn color_option_roundtrip_some() {
    let ffi = FfiColor::from_option(Some(Color::Red));
    assert_eq!(ffi.tag, 10);
    assert_eq!(ffi.into_option(), Some(Color::Red));
}

#[test]
fn color_unknown_tag_falls_back_to_reset() {
    let ffi = FfiColor {
        tag: 200,
        r: 0,
        g: 0,
        b: 0,
    };
    assert_eq!(ffi.into_color(), Color::Reset);
}

#[test]
fn style_roundtrip_default() {
    let s = Style::default();
    let ffi = FfiStyle::from_style(&s);
    assert_eq!(ffi, FfiStyle::DEFAULT);
    assert_eq!(ffi.into_style(), s);
}

#[test]
fn style_roundtrip_all_attributes() {
    let attrs = Attributes::BOLD
        | Attributes::ITALIC
        | Attributes::UNDERLINE
        | Attributes::STRIKETHROUGH
        | Attributes::REVERSE
        | Attributes::DIM;
    let s = Style {
        fg: Some(Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        }),
        bg: Some(Color::AnsiValue(42)),
        attributes: attrs,
    };
    let ffi = FfiStyle::from_style(&s);
    let back = ffi.into_style();
    assert_eq!(back.fg, s.fg);
    assert_eq!(back.bg, s.bg);
    assert_eq!(back.attributes.bits(), attrs.bits());
}

#[test]
fn style_unknown_attribute_bits_preserved() {
    let ffi = FfiStyle {
        fg: FfiColor::NONE,
        bg: FfiColor::NONE,
        attributes: 0b1111_1111,
        _pad: [0; 3],
    };
    assert_eq!(ffi.into_style().attributes.bits(), 0b1111_1111);
}

// =============================================================================
// T-1 Round-trip tests — remaining boundary types
// =============================================================================

#[test]
fn platform_caps_snapshot_roundtrip_all_fields() {
    let caps = MockPlatformCapabilities {
        rendering_model: RenderingModel::Canvas,
        grid_size: Some((132, 48)),
        color_depth: ColorDepth::Ansi256,
        pixel_size: Some((1920, 1080)),
        reliable_unicode_width: false,
        dark_mode: false,
        smooth_scroll: true,
        pointer_events: true,
        touch_input: true,
        haptic: true,
        safe_area: Insets {
            top: 1,
            right: 2,
            bottom: 3,
            left: 4,
        },
        has_focus: false,
        clipboard_available: false,
        screen_reader_active: true,
    };

    let snap = FfiPlatformCaps::snapshot(&caps);
    assert_eq!(snap.rendering_model, 1);
    assert_eq!(snap.color_depth, 2);
    assert_eq!(snap.grid_cols, 132);
    assert_eq!(snap.grid_rows, 48);
    assert_eq!(snap.has_grid_size, 1);
    assert_eq!(snap.pixel_w, 1920);
    assert_eq!(snap.pixel_h, 1080);
    assert_eq!(snap.has_pixel_size, 1);
    assert_eq!(snap.safe_area, caps.safe_area);

    let ffi_caps = FfiCapsImpl::new(&snap);
    assert_eq!(ffi_caps.rendering_model(), caps.rendering_model);
    assert_eq!(ffi_caps.grid_size(), caps.grid_size);
    assert_eq!(ffi_caps.color_depth(), caps.color_depth);
    assert_eq!(ffi_caps.pixel_size(), caps.pixel_size);
    assert_eq!(ffi_caps.reliable_unicode_width(), caps.reliable_unicode_width);
    assert_eq!(ffi_caps.dark_mode(), caps.dark_mode);
    assert_eq!(ffi_caps.smooth_scroll(), caps.smooth_scroll);
    assert_eq!(ffi_caps.pointer_events(), caps.pointer_events);
    assert_eq!(ffi_caps.touch_input(), caps.touch_input);
    assert_eq!(ffi_caps.haptic(), caps.haptic);
    assert_eq!(ffi_caps.safe_area(), caps.safe_area);
    assert_eq!(ffi_caps.has_focus(), caps.has_focus);
    assert_eq!(ffi_caps.clipboard_available(), caps.clipboard_available);
    assert_eq!(ffi_caps.screen_reader_active(), caps.screen_reader_active);
}

#[test]
fn platform_caps_snapshot_handles_missing_sizes() {
    let mut caps = MockPlatformCapabilities::new();
    caps.grid_size = None;
    caps.pixel_size = None;

    let snap = FfiPlatformCaps::snapshot(&caps);
    let ffi_caps = FfiCapsImpl::new(&snap);
    assert_eq!(snap.has_grid_size, 0);
    assert_eq!(snap.has_pixel_size, 0);
    assert_eq!(ffi_caps.grid_size(), None);
    assert_eq!(ffi_caps.pixel_size(), None);
}

#[test]
fn annotation_context_roundtrip() {
    let ctx = AnnotationContext {
        buffer_id: BufferId(42),
        total_lines: 300,
        visible_range: (10, 44),
        cursor_line: 17,
        gutter_style: sample_style(),
    };
    let ffi = FfiAnnotationContext::from_ctx(&ctx);
    let back = ffi.into_ctx();
    assert_eq!(back.buffer_id.0, 42);
    assert_eq!(back.total_lines, 300);
    assert_eq!(back.visible_range, (10, 44));
    assert_eq!(back.cursor_line, 17);
    assert_eq!(back.gutter_style, ctx.gutter_style);
}

#[test]
fn gutter_cell_roundtrip_short_text() {
    let cell = GutterCell {
        text: "ln".to_string(),
        style: sample_style(),
    };
    let ffi = FfiGutterCell::from_cell(&cell);
    assert_eq!(ffi.into_cell(), Some(cell));
}

#[test]
fn gutter_cell_empty_encodes_none() {
    let cell = GutterCell {
        text: String::new(),
        style: Style::default(),
    };
    let ffi = FfiGutterCell::from_cell(&cell);
    assert!(ffi.is_none());
    assert_eq!(ffi.into_cell(), None);
}

#[test]
fn gutter_cell_truncates_to_char_boundary() {
    let cell = GutterCell {
        text: "é".repeat(16),
        style: Style::default(),
    };
    let ffi = FfiGutterCell::from_cell(&cell);
    assert_eq!(usize::from(ffi.text_len), 30);
    let back = ffi.into_cell().expect("truncated text still decodes");
    assert_eq!(back.text, "é".repeat(15));
}

#[test]
fn inline_decoration_roundtrip() {
    let deco = InlineDecoration {
        col_start: 3,
        col_end: 9,
        style: sample_style(),
    };
    let ffi = FfiInlineDecoration::from_deco(&deco);
    assert_eq!(ffi.into_deco(), deco);
}

#[test]
fn column_width_roundtrip_variants() {
    assert_eq!(
        FfiColumnWidth::from_width(ColumnWidth::Fixed(0)).into_width(),
        ColumnWidth::Fixed(0)
    );
    assert_eq!(
        FfiColumnWidth::from_width(ColumnWidth::Fixed(5)).into_width(),
        ColumnWidth::Fixed(5)
    );
    assert_eq!(
        FfiColumnWidth::from_width(ColumnWidth::Dynamic(12)).into_width(),
        ColumnWidth::Dynamic(12)
    );
}

#[test]
fn fold_range_preserves_values() {
    let ffi = FfiFoldRange {
        start_line: 0,
        line_count: usize::MAX / 2,
    };
    assert_eq!(ffi.start_line, 0);
    assert_eq!(ffi.line_count, usize::MAX / 2);
}

#[test]
fn virtual_line_roundtrip_short_content() {
    let line = VirtualLine {
        buffer_line: 12,
        position: VirtualLinePosition::Before,
        content: "note".to_string(),
        style: sample_style(),
    };
    let ffi = FfiVirtualLine::from_rust(&line);
    assert_eq!(ffi.into_rust(), line);
}

#[test]
fn virtual_line_truncates_at_255_bytes() {
    let line = VirtualLine {
        buffer_line: 1,
        position: VirtualLinePosition::After,
        content: "a".repeat(300),
        style: Style::default(),
    };
    let ffi = FfiVirtualLine::from_rust(&line);
    assert_eq!(usize::from(ffi.content_len), 255);
    let back = ffi.into_rust();
    assert_eq!(back.buffer_line, 1);
    assert_eq!(back.position, VirtualLinePosition::After);
    assert_eq!(back.content.len(), 255);
}

#[test]
fn transformed_line_roundtrip_empty_segments() {
    let _guard = TRANSFORMED_LINE_TEST_LOCK.lock().unwrap();
    let before_allocs = TRANSFORMED_LINE_ALLOC_COUNT.load(Ordering::Relaxed);
    let before_frees = TRANSFORMED_LINE_FREE_COUNT.load(Ordering::Relaxed);
    let ptr = FfiTransformedLine::from_rust(TransformedLine {
        segments: Vec::new(),
    });
    let owned = unsafe { FfiTransformedLine::read_owned(ptr) };
    assert!(owned.segments.is_empty());
    unsafe { FfiTransformedLine::free(ptr) };
    assert_eq!(TRANSFORMED_LINE_ALLOC_COUNT.load(Ordering::Relaxed), before_allocs + 1);
    assert_eq!(TRANSFORMED_LINE_FREE_COUNT.load(Ordering::Relaxed), before_frees + 1);
}

#[test]
fn transformed_line_read_owned_and_free_track_allocator_lifecycle() {
    let _guard = TRANSFORMED_LINE_TEST_LOCK.lock().unwrap();
    let before_allocs = TRANSFORMED_LINE_ALLOC_COUNT.load(Ordering::Relaxed);
    let before_frees = TRANSFORMED_LINE_FREE_COUNT.load(Ordering::Relaxed);
    let transformed = TransformedLine {
        segments: vec![
            ("αβ".to_string(), Some(sample_style())),
            ("tail".to_string(), None),
        ],
    };

    let ptr = FfiTransformedLine::from_rust(transformed.clone());
    assert_eq!(TRANSFORMED_LINE_ALLOC_COUNT.load(Ordering::Relaxed), before_allocs + 1);

    let owned = unsafe { FfiTransformedLine::read_owned(ptr) };
    assert_eq!(owned, transformed);
    assert_eq!(TRANSFORMED_LINE_FREE_COUNT.load(Ordering::Relaxed), before_frees);

    unsafe { FfiTransformedLine::free(ptr) };
    assert_eq!(TRANSFORMED_LINE_FREE_COUNT.load(Ordering::Relaxed), before_frees + 1);
}

#[test]
fn render_behavior_roundtrip_all_variants() {
    let cases = [
        None,
        Some(RenderBehavior::Highlight),
        Some(RenderBehavior::Conceal {
            replacement: Cow::Borrowed("·"),
        }),
        Some(RenderBehavior::Background(Color::Magenta)),
        Some(RenderBehavior::Hide),
        Some(RenderBehavior::FullWidthLine {
            ch: '─',
            style: sample_style(),
        }),
    ];

    for case in cases {
        let ffi = FfiRenderBehavior::from_option(case.clone());
        assert_eq!(ffi.into_option(), case);
    }
}

// =============================================================================
// T-4 Vtable dispatch
// =============================================================================

#[test]
fn render_surface_vtable_dispatch_calls_all_host_operations() {
    let mut mock = MockRenderSurface::with_size(120, 55);
    let mut host = FfiRenderSurfaceHost::new(&mut mock);
    let ffi = FfiRenderSurface::from_host(&mut host);
    let style = sample_style();
    let text = "ffi";

    let written = unsafe {
        (ffi.write_styled)(
            ffi.opaque,
            3,
            4,
            text.as_ptr(),
            text.len(),
            FfiStyle::from_style(&style),
        )
    };
    assert_eq!(written, 3);
    unsafe {
        (ffi.apply_style)(ffi.opaque, 5, 6, FfiStyle::from_style(&style));
    }
    unsafe {
        (ffi.overlay_bg)(ffi.opaque, 7, 8, FfiColor::from_color(Color::Blue));
    }
    unsafe {
        (ffi.fill)(
            ffi.opaque,
            Rect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
            '█' as u32,
            FfiStyle::from_style(&style),
        );
    };
    unsafe {
        (ffi.clear)(
            ffi.opaque,
            Rect {
                x: 9,
                y: 10,
                width: 11,
                height: 12,
            },
        );
    };
    let packed = unsafe { (ffi.size)(ffi.opaque) };
    assert_eq!(((packed >> 16) as u16, (packed & 0xFFFF) as u16), (120, 55));

    assert_eq!(
        mock.ops,
        vec![
            RenderOp::Write {
                x: 3,
                y: 4,
                text: "ffi".to_string(),
                style: style.clone(),
            },
            RenderOp::ApplyStyle {
                x: 5,
                y: 6,
                style: style.clone(),
            },
            RenderOp::OverlayBg {
                x: 7,
                y: 8,
                bg: Color::Blue,
            },
            RenderOp::Fill {
                rect: Rect {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4,
                },
                ch: '█',
                style,
            },
            RenderOp::Clear {
                rect: Rect {
                    x: 9,
                    y: 10,
                    width: 11,
                    height: 12,
                },
            },
        ],
    );
}

#[test]
fn theme_provider_vtable_dispatch_roundtrips_host_values() {
    let emphasis = Style::new().fg(Color::Yellow).bold();
    let theme = MockThemeProvider::new().with_highlight("Comment", emphasis.clone());
    let host = FfiThemeProviderHost::new(&theme);
    let ffi = FfiThemeProvider::from_host(&host);

    let group = "Comment";
    let highlight = unsafe { (ffi.highlight)(ffi.opaque, group.as_ptr(), group.len()) };
    assert_eq!(highlight.into_style(), emphasis);
    assert_eq!(unsafe { (ffi.foreground)(ffi.opaque) }.into_style().fg, Some(Color::White));
    assert_eq!(unsafe { (ffi.background)(ffi.opaque) }.into_style().bg, Some(Color::Black));
    assert_eq!(unsafe { (ffi.is_dark)(ffi.opaque) }, 1);

    let theme_ref = FfiThemeRef::new(&ffi);
    assert_eq!(theme_ref.highlight("Comment"), Style::new().fg(Color::Yellow).bold());
    assert_eq!(
        theme_ref.highlight_with_fallback(&["Missing", "Comment"]),
        Style::new().fg(Color::Yellow).bold(),
    );
}
