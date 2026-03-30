use std::error::Error;

use super::*;

// =============================================================================
// ProbeResult
// =============================================================================

#[test]
fn probe_result_success() {
    let result = ProbeResult::Success;
    assert!(matches!(result, ProbeResult::Success));
}

#[test]
fn probe_result_defer() {
    let result = ProbeResult::Defer("not ready".to_string());
    assert!(matches!(result, ProbeResult::Defer(ref s) if s == "not ready"));
}

#[test]
fn probe_result_failed() {
    let err = ClientModuleError::other("boom");
    let result = ProbeResult::Failed(err);
    assert!(matches!(result, ProbeResult::Failed(ref e) if e.message() == "boom"));
}

// =============================================================================
// ClientModuleError
// =============================================================================

#[test]
fn client_module_error_other() {
    let err = ClientModuleError::other("test error");
    assert_eq!(err.message(), "test error");
    assert_eq!(err.to_string(), "test error");
}

#[test]
fn client_module_error_init_failed() {
    let err = ClientModuleError::init_failed("no server", None);
    assert_eq!(err.message(), "no server");
    assert_eq!(err.to_string(), "init failed: no server");
    assert!(err.source().is_none());
}

#[test]
fn client_module_error_init_failed_with_source() {
    let source = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let err = ClientModuleError::init_failed("config load failed", Some(Box::new(source)));
    assert_eq!(err.message(), "config load failed");
    assert!(err.source().is_some());
}

#[test]
fn client_module_error_exit_failed() {
    let err = ClientModuleError::exit_failed("cleanup error");
    assert_eq!(err.message(), "cleanup error");
    assert_eq!(err.to_string(), "exit failed: cleanup error");
    assert!(err.source().is_none());
}

#[test]
fn client_module_error_notification_parse() {
    let err = ClientModuleError::notification_parse("whichkey", "invalid JSON");
    assert_eq!(err.message(), "invalid JSON");
    assert_eq!(err.to_string(), "notification parse error (whichkey): invalid JSON");
    assert!(err.source().is_none());
}

#[test]
fn client_module_error_empty_message() {
    let err = ClientModuleError::other("");
    assert!(err.message().is_empty());
}

#[test]
fn client_module_error_debug() {
    let err = ClientModuleError::other("debug test");
    let debug = format!("{err:?}");
    assert!(debug.contains("Other"));
    assert!(debug.contains("debug test"));
}

// =============================================================================
// Version
// =============================================================================

#[test]
fn version_construction() {
    let v = Version::new(1, 2, 3);
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 2);
    assert_eq!(v.patch, 3);
}

#[test]
fn version_equality() {
    let a = Version::new(1, 0, 0);
    let b = Version::new(1, 0, 0);
    assert_eq!(a, b);
}

#[test]
fn version_inequality() {
    let a = Version::new(1, 0, 0);
    let b = Version::new(2, 0, 0);
    assert_ne!(a, b);
}

#[test]
fn version_display() {
    let v = Version::new(0, 12, 3);
    assert_eq!(v.to_string(), "0.12.3");
}

#[test]
fn version_copy_semantics() {
    let a = Version::new(1, 0, 0);
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// BufferId
// =============================================================================

#[test]
fn buffer_id_construction() {
    let id = BufferId(42);
    assert_eq!(id.0, 42);
}

#[test]
fn buffer_id_equality() {
    assert_eq!(BufferId(1), BufferId(1));
    assert_ne!(BufferId(1), BufferId(2));
}

#[test]
fn buffer_id_copy() {
    let a = BufferId(5);
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn buffer_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(BufferId(1));
    set.insert(BufferId(2));
    set.insert(BufferId(1));
    assert_eq!(set.len(), 2);
}

// =============================================================================
// ColorDepth
// =============================================================================

#[test]
fn color_depth_variants() {
    let depths = [
        ColorDepth::Monochrome,
        ColorDepth::Ansi16,
        ColorDepth::Ansi256,
        ColorDepth::TrueColor,
    ];
    for (i, d) in depths.iter().enumerate() {
        for (j, other) in depths.iter().enumerate() {
            if i == j {
                assert_eq!(d, other);
            } else {
                assert_ne!(d, other);
            }
        }
    }
}

// =============================================================================
// RenderingModel
// =============================================================================

#[test]
fn rendering_model_variants() {
    let models = [
        RenderingModel::CellGrid,
        RenderingModel::Canvas,
        RenderingModel::NativeLayout,
    ];
    for (i, m) in models.iter().enumerate() {
        for (j, other) in models.iter().enumerate() {
            if i == j {
                assert_eq!(m, other);
            } else {
                assert_ne!(m, other);
            }
        }
    }
}

// =============================================================================
// Rect
// =============================================================================

#[test]
fn rect_construction() {
    let r = Rect::new(1, 2, 80, 24);
    assert_eq!(r.x, 1);
    assert_eq!(r.y, 2);
    assert_eq!(r.width, 80);
    assert_eq!(r.height, 24);
}

#[test]
fn rect_default() {
    let r = Rect::default();
    assert_eq!(r, Rect::new(0, 0, 0, 0));
}

#[test]
fn rect_equality() {
    assert_eq!(Rect::new(0, 0, 10, 20), Rect::new(0, 0, 10, 20));
    assert_ne!(Rect::new(0, 0, 10, 20), Rect::new(0, 0, 10, 21));
}

#[test]
fn rect_copy() {
    let a = Rect::new(1, 2, 3, 4);
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// Insets
// =============================================================================

#[test]
fn insets_construction() {
    let i = Insets::new(1, 2, 3, 4);
    assert_eq!(i.top, 1);
    assert_eq!(i.bottom, 2);
    assert_eq!(i.left, 3);
    assert_eq!(i.right, 4);
}

#[test]
fn insets_default() {
    let i = Insets::default();
    assert_eq!(i, Insets::new(0, 0, 0, 0));
}

#[test]
fn insets_copy() {
    let a = Insets::new(1, 2, 3, 4);
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// OptionValue
// =============================================================================

#[test]
fn option_value_bool() {
    let v = OptionValue::Bool(true);
    assert_eq!(v, OptionValue::Bool(true));
    assert_ne!(v, OptionValue::Bool(false));
}

#[test]
fn option_value_integer() {
    let v = OptionValue::Integer(42);
    assert_eq!(v, OptionValue::Integer(42));
}

#[test]
fn option_value_string() {
    let v = OptionValue::String("hello".to_string());
    assert_eq!(v, OptionValue::String("hello".to_string()));
}

#[test]
fn option_value_cross_variant_inequality() {
    assert_ne!(OptionValue::Bool(true), OptionValue::Integer(1));
}

// =============================================================================
// OptionKind
// =============================================================================

#[test]
fn option_kind_variants() {
    let kinds = [OptionKind::Bool, OptionKind::Integer, OptionKind::String];
    for (i, k) in kinds.iter().enumerate() {
        for (j, other) in kinds.iter().enumerate() {
            if i == j {
                assert_eq!(k, other);
            } else {
                assert_ne!(k, other);
            }
        }
    }
}

#[test]
fn option_kind_copy() {
    let a = OptionKind::Bool;
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// OptionMetadata
// =============================================================================

#[test]
fn option_metadata_construction() {
    let meta = OptionMetadata {
        name: "number".to_string(),
        description: "Show line numbers".to_string(),
        default_value: Some(OptionValue::Bool(false)),
        kind: OptionKind::Bool,
    };
    assert_eq!(meta.name, "number");
    assert_eq!(meta.kind, OptionKind::Bool);
    assert_eq!(meta.default_value, Some(OptionValue::Bool(false)));
}

#[test]
fn option_metadata_no_default() {
    let meta = OptionMetadata {
        name: "custom".to_string(),
        description: "A custom option".to_string(),
        default_value: None,
        kind: OptionKind::String,
    };
    assert!(meta.default_value.is_none());
}

#[test]
fn option_metadata_clone_eq() {
    let a = OptionMetadata {
        name: "tabstop".to_string(),
        description: "Tab width".to_string(),
        default_value: Some(OptionValue::Integer(8)),
        kind: OptionKind::Integer,
    };
    let b = a.clone();
    assert_eq!(a, b);
}

// =============================================================================
// RenderBehavior
// =============================================================================

#[test]
fn render_behavior_highlight() {
    assert_eq!(RenderBehavior::Highlight, RenderBehavior::Highlight);
}

#[test]
fn render_behavior_conceal() {
    let rb = RenderBehavior::Conceal {
        replacement: Cow::Borrowed("..."),
    };
    assert!(matches!(rb, RenderBehavior::Conceal { replacement } if replacement == "..."));
}

#[test]
fn render_behavior_background() {
    let rb = RenderBehavior::Background(Color::Red);
    assert!(matches!(rb, RenderBehavior::Background(Color::Red)));
}

#[test]
fn render_behavior_hide() {
    assert_eq!(RenderBehavior::Hide, RenderBehavior::Hide);
}

#[test]
fn render_behavior_full_width_line() {
    let rb = RenderBehavior::FullWidthLine {
        ch: '-',
        style: Style::default(),
    };
    assert!(matches!(rb, RenderBehavior::FullWidthLine { ch: '-', .. }));
}

// =============================================================================
// TransformedLine
// =============================================================================

#[test]
fn transformed_line_construction() {
    let tl = TransformedLine {
        segments: vec![("hello".to_string(), None)],
    };
    assert_eq!(tl.segments.len(), 1);
    assert_eq!(tl.segments[0].0, "hello");
    assert!(tl.segments[0].1.is_none());
}

#[test]
fn transformed_line_empty_segments() {
    let tl = TransformedLine {
        segments: Vec::new(),
    };
    assert!(tl.segments.is_empty());
}

#[test]
fn transformed_line_multi_segment() {
    let style = Style {
        fg: Some(Color::Red),
        bg: None,
        attributes: Attributes::BOLD,
    };
    let tl = TransformedLine {
        segments: vec![
            ("hello ".to_string(), Some(style.clone())),
            ("world".to_string(), None),
        ],
    };
    assert_eq!(tl.segments.len(), 2);
    assert_eq!(tl.segments[0].1, Some(style));
}

// =============================================================================
// VirtualLine
// =============================================================================

#[test]
fn virtual_line_construction() {
    let vl = VirtualLine {
        buffer_line: 10,
        position: VirtualLinePosition::Before,
        content: "diagnostic".to_string(),
        style: Style::default(),
    };
    assert_eq!(vl.buffer_line, 10);
    assert_eq!(vl.position, VirtualLinePosition::Before);
    assert_eq!(vl.content, "diagnostic");
}

#[test]
fn virtual_line_position_variants() {
    assert_ne!(VirtualLinePosition::Before, VirtualLinePosition::After);
}

// =============================================================================
// InlineDecoration
// =============================================================================

#[test]
fn inline_decoration_construction() {
    let dec = InlineDecoration {
        col_start: 5,
        col_end: 10,
        style: Style::default(),
    };
    assert_eq!(dec.col_start, 5);
    assert_eq!(dec.col_end, 10);
}

// =============================================================================
// ChromePosition
// =============================================================================

#[test]
fn chrome_position_variants() {
    let positions = [
        ChromePosition::Top,
        ChromePosition::Bottom,
        ChromePosition::Left,
        ChromePosition::Right,
        ChromePosition::Overlay,
    ];
    for (i, p) in positions.iter().enumerate() {
        for (j, other) in positions.iter().enumerate() {
            if i == j {
                assert_eq!(p, other);
            } else {
                assert_ne!(p, other);
            }
        }
    }
}

#[test]
fn chrome_position_copy() {
    let a = ChromePosition::Left;
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// AnnotationContext
// =============================================================================

#[test]
fn annotation_context_construction() {
    let ctx = AnnotationContext {
        buffer_id: BufferId(1),
        total_lines: 100,
        visible_range: (0, 40),
        cursor_line: 15,
        gutter_style: Style::default(),
    };
    assert_eq!(ctx.buffer_id, BufferId(1));
    assert_eq!(ctx.total_lines, 100);
    assert_eq!(ctx.visible_range, (0, 40));
    assert_eq!(ctx.cursor_line, 15);
}

// =============================================================================
// ColumnWidth
// =============================================================================

#[test]
fn column_width_fixed() {
    let w = ColumnWidth::Fixed(4);
    assert_eq!(w, ColumnWidth::Fixed(4));
    assert_ne!(w, ColumnWidth::Fixed(5));
}

#[test]
fn column_width_dynamic() {
    let w = ColumnWidth::Dynamic(2);
    assert_eq!(w, ColumnWidth::Dynamic(2));
}

#[test]
fn column_width_cross_variant() {
    assert_ne!(ColumnWidth::Fixed(4), ColumnWidth::Dynamic(4));
}

// =============================================================================
// GutterCell
// =============================================================================

#[test]
fn gutter_cell_construction() {
    let cell = GutterCell {
        text: "42".to_string(),
        style: Style::default(),
    };
    assert_eq!(cell.text, "42");
}

// =============================================================================
// BufferUpdateEvent
// =============================================================================

#[test]
fn buffer_update_event_construction() {
    let event = BufferUpdateEvent {
        buffer_id: BufferId(1),
        revision: 5,
        changed_range: 0..10,
        new_lines: vec!["line1".to_string()],
        total_lines: 100,
    };
    assert_eq!(event.buffer_id, BufferId(1));
    assert_eq!(event.revision, 5);
    assert_eq!(event.changed_range, 0..10);
    assert_eq!(event.new_lines.len(), 1);
    assert_eq!(event.total_lines, 100);
}

// =============================================================================
// WindowId
// =============================================================================

#[test]
fn window_id_construction() {
    let id = WindowId(3);
    assert_eq!(id.0, 3);
}

#[test]
fn window_id_equality() {
    assert_eq!(WindowId(1), WindowId(1));
    assert_ne!(WindowId(1), WindowId(2));
}

#[test]
fn window_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(WindowId(1));
    set.insert(WindowId(2));
    set.insert(WindowId(1));
    assert_eq!(set.len(), 2);
}

// =============================================================================
// WindowLayout
// =============================================================================

#[test]
fn window_layout_construction() {
    let layout = WindowLayout {
        window_id: WindowId(1),
        bounds: Rect::new(0, 0, 80, 24),
    };
    assert_eq!(layout.window_id, WindowId(1));
    assert_eq!(layout.bounds, Rect::new(0, 0, 80, 24));
}

// =============================================================================
// Attributes
// =============================================================================

#[test]
fn attributes_new_is_empty() {
    let a = Attributes::new();
    assert!(a.is_empty());
    assert_eq!(a.bits(), 0);
}

#[test]
fn attributes_default_is_empty() {
    assert_eq!(Attributes::default(), Attributes::new());
}

#[test]
fn attributes_set_and_contains() {
    let mut a = Attributes::new();
    assert!(!a.contains(Attributes::BOLD));
    a.set(Attributes::BOLD);
    assert!(a.contains(Attributes::BOLD));
    assert!(!a.contains(Attributes::ITALIC));
}

#[test]
fn attributes_unset() {
    let mut a = Attributes::new();
    a.set(Attributes::BOLD);
    a.set(Attributes::ITALIC);
    assert!(a.contains(Attributes::BOLD));
    a.unset(Attributes::BOLD);
    assert!(!a.contains(Attributes::BOLD));
    assert!(a.contains(Attributes::ITALIC));
}

#[test]
fn attributes_all_flags() {
    let flags = [
        Attributes::BOLD,
        Attributes::ITALIC,
        Attributes::UNDERLINE,
        Attributes::STRIKETHROUGH,
        Attributes::REVERSE,
        Attributes::DIM,
    ];
    for flag in &flags {
        let mut a = Attributes::new();
        a.set(*flag);
        assert!(a.contains(*flag));
        assert!(!a.is_empty());
    }
}

#[test]
fn attributes_bitor() {
    let combined = Attributes::BOLD | Attributes::ITALIC;
    assert!(combined.contains(Attributes::BOLD));
    assert!(combined.contains(Attributes::ITALIC));
    assert!(!combined.contains(Attributes::UNDERLINE));
}

#[test]
fn attributes_bitand() {
    let a = Attributes::BOLD | Attributes::ITALIC;
    let b = Attributes::BOLD | Attributes::UNDERLINE;
    let intersection = a & b;
    assert!(intersection.contains(Attributes::BOLD));
    assert!(!intersection.contains(Attributes::ITALIC));
    assert!(!intersection.contains(Attributes::UNDERLINE));
}

#[test]
fn attributes_contains_empty_in_nonempty() {
    let a = Attributes::BOLD;
    assert!(a.contains(Attributes::new()));
}

#[test]
fn attributes_multiple_flags_combined() {
    let mut a = Attributes::new();
    a.set(Attributes::BOLD);
    a.set(Attributes::ITALIC);
    a.set(Attributes::UNDERLINE);
    assert!(a.contains(Attributes::BOLD | Attributes::ITALIC));
}

// =============================================================================
// Style
// =============================================================================

#[test]
fn style_new() {
    let s = Style::new();
    assert!(s.fg.is_none());
    assert!(s.bg.is_none());
    assert!(s.attributes.is_empty());
}

#[test]
fn style_default() {
    assert_eq!(Style::default(), Style::new());
}

#[test]
fn style_with_colors() {
    let s = Style {
        fg: Some(Color::Red),
        bg: Some(Color::Blue),
        attributes: Attributes::BOLD,
    };
    assert_eq!(s.fg, Some(Color::Red));
    assert_eq!(s.bg, Some(Color::Blue));
    assert!(s.attributes.contains(Attributes::BOLD));
}

#[test]
fn style_equality() {
    let a = Style {
        fg: Some(Color::Red),
        bg: None,
        attributes: Attributes::BOLD,
    };
    let b = Style {
        fg: Some(Color::Red),
        bg: None,
        attributes: Attributes::BOLD,
    };
    assert_eq!(a, b);
}

#[test]
fn style_inequality_fg() {
    let a = Style {
        fg: Some(Color::Red),
        ..Style::default()
    };
    let b = Style {
        fg: Some(Color::Blue),
        ..Style::default()
    };
    assert_ne!(a, b);
}

#[test]
fn style_inequality_attributes() {
    let a = Style {
        attributes: Attributes::BOLD,
        ..Style::default()
    };
    let b = Style {
        attributes: Attributes::ITALIC,
        ..Style::default()
    };
    assert_ne!(a, b);
}

// =============================================================================
// InputEvent
// =============================================================================

#[test]
fn input_event_key_variant() {
    let event = InputEvent::Key(KeyEvent::new(KeyCode::Char('a'), Modifiers::NONE));
    assert!(matches!(event, InputEvent::Key(_)));
    assert!(!format!("{event:?}").is_empty());
}

#[test]
fn input_event_pointer_variant() {
    let event = InputEvent::Pointer(PointerEvent {
        kind: PointerKind::Down(PointerButton::Left),
        x: 10,
        y: 20,
        modifiers: Modifiers::NONE,
    });
    assert!(matches!(event, InputEvent::Pointer(_)));
}

#[test]
fn input_event_touch_variant() {
    let event = InputEvent::Touch(TouchEvent {
        kind: TouchKind::Start,
        id: 1,
        x: 100.0,
        y: 200.0,
    });
    assert!(matches!(event, InputEvent::Touch(_)));
}

#[test]
fn input_event_focus_variant() {
    let gained = InputEvent::Focus(FocusEvent::Gained);
    let lost = InputEvent::Focus(FocusEvent::Lost);
    assert!(matches!(gained, InputEvent::Focus(FocusEvent::Gained)));
    assert!(matches!(lost, InputEvent::Focus(FocusEvent::Lost)));
}

#[test]
fn input_event_paste_variant() {
    let event = InputEvent::Paste("hello".to_string());
    assert!(matches!(event, InputEvent::Paste(_)));
    let cloned = event.clone();
    assert_eq!(event, cloned);
}

// =============================================================================
// KeyEvent
// =============================================================================

#[test]
fn key_event_construction() {
    let event = KeyEvent::new(KeyCode::Enter, Modifiers::CTRL);
    assert_eq!(event.code, KeyCode::Enter);
    assert!(event.modifiers.contains(Modifiers::CTRL));
}

#[test]
fn key_event_clone_eq() {
    let a = KeyEvent::new(KeyCode::Esc, Modifiers::ALT);
    let b = a.clone();
    assert_eq!(a, b);
}

// =============================================================================
// KeyCode
// =============================================================================

#[test]
fn key_code_char_variants() {
    assert_eq!(KeyCode::Char('a'), KeyCode::Char('a'));
    assert_ne!(KeyCode::Char('a'), KeyCode::Char('b'));
}

#[test]
fn key_code_special_keys() {
    let keys = [
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Tab,
        KeyCode::Backspace,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Insert,
        KeyCode::Delete,
        KeyCode::Null,
    ];
    for key in keys {
        assert!(!format!("{key:?}").is_empty());
    }
}

#[test]
fn key_code_function_keys() {
    assert_eq!(KeyCode::F(1), KeyCode::F(1));
    assert_ne!(KeyCode::F(1), KeyCode::F(12));
    let f0 = KeyCode::F(0);
    assert!(!format!("{f0:?}").is_empty());
}

// =============================================================================
// Modifiers
// =============================================================================

#[test]
fn modifiers_empty() {
    let m = Modifiers::new();
    assert!(m.is_empty());
    assert_eq!(m.bits(), 0);
    assert_eq!(m, Modifiers::NONE);
}

#[test]
fn modifiers_set_unset() {
    let mut m = Modifiers::new();
    m.set(Modifiers::SHIFT);
    assert!(m.contains(Modifiers::SHIFT));
    assert!(!m.contains(Modifiers::CTRL));

    m.set(Modifiers::CTRL);
    assert!(m.contains(Modifiers::SHIFT));
    assert!(m.contains(Modifiers::CTRL));

    m.unset(Modifiers::SHIFT);
    assert!(!m.contains(Modifiers::SHIFT));
    assert!(m.contains(Modifiers::CTRL));
}

#[test]
fn modifiers_bitor() {
    let m = Modifiers::SHIFT | Modifiers::CTRL;
    assert!(m.contains(Modifiers::SHIFT));
    assert!(m.contains(Modifiers::CTRL));
    assert!(!m.contains(Modifiers::ALT));
}

#[test]
fn modifiers_bitand() {
    let m = Modifiers::SHIFT | Modifiers::CTRL | Modifiers::ALT;
    let masked = m & Modifiers::CTRL;
    assert!(masked.contains(Modifiers::CTRL));
    assert!(!masked.contains(Modifiers::SHIFT));
}

#[test]
fn modifiers_all_flags() {
    let all = Modifiers::SHIFT | Modifiers::CTRL | Modifiers::ALT | Modifiers::SUPER;
    assert!(all.contains(Modifiers::SHIFT));
    assert!(all.contains(Modifiers::CTRL));
    assert!(all.contains(Modifiers::ALT));
    assert!(all.contains(Modifiers::SUPER));
    assert!(!all.is_empty());
}

#[test]
fn modifiers_default() {
    let m = Modifiers::default();
    assert!(m.is_empty());
    assert_eq!(m, Modifiers::NONE);
}

// =============================================================================
// PointerEvent
// =============================================================================

#[test]
fn pointer_event_construction() {
    let event = PointerEvent {
        kind: PointerKind::Down(PointerButton::Left),
        x: 5,
        y: 10,
        modifiers: Modifiers::SHIFT,
    };
    assert_eq!(event.x, 5);
    assert_eq!(event.y, 10);
    assert!(event.modifiers.contains(Modifiers::SHIFT));
}

#[test]
fn pointer_kind_all_variants() {
    let variants = [
        PointerKind::Down(PointerButton::Left),
        PointerKind::Down(PointerButton::Right),
        PointerKind::Down(PointerButton::Middle),
        PointerKind::Up(PointerButton::Left),
        PointerKind::Drag(PointerButton::Left),
        PointerKind::Move,
        PointerKind::ScrollUp,
        PointerKind::ScrollDown,
    ];
    for v in variants {
        assert!(!format!("{v:?}").is_empty());
    }
}

#[test]
fn pointer_event_at_origin() {
    let event = PointerEvent {
        kind: PointerKind::Move,
        x: 0,
        y: 0,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(event.x, 0);
    assert_eq!(event.y, 0);
}

#[test]
fn pointer_event_at_max() {
    let event = PointerEvent {
        kind: PointerKind::ScrollUp,
        x: u16::MAX,
        y: u16::MAX,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(event.x, u16::MAX);
    assert_eq!(event.y, u16::MAX);
}

// =============================================================================
// TouchEvent
// =============================================================================

#[test]
fn touch_event_construction() {
    let event = TouchEvent {
        kind: TouchKind::Start,
        id: 42,
        x: 1.5,
        y: 2.5,
    };
    assert_eq!(event.id, 42);
    assert!((event.x - 1.5).abs() < f32::EPSILON);
    assert!((event.y - 2.5).abs() < f32::EPSILON);
}

#[test]
fn touch_kind_all_variants() {
    let variants = [
        TouchKind::Start,
        TouchKind::Move,
        TouchKind::End,
        TouchKind::Cancel,
    ];
    for v in variants {
        assert!(!format!("{v:?}").is_empty());
    }
}

#[test]
fn touch_event_clone() {
    let a = TouchEvent {
        kind: TouchKind::End,
        id: 1,
        x: 0.0,
        y: 0.0,
    };
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// FocusEvent
// =============================================================================

#[test]
fn focus_event_variants() {
    assert_ne!(FocusEvent::Gained, FocusEvent::Lost);
    let gained = FocusEvent::Gained;
    let cloned = gained;
    assert_eq!(gained, cloned);
}

#[test]
fn focus_event_debug() {
    assert!(format!("{:?}", FocusEvent::Gained).contains("Gained"));
    assert!(format!("{:?}", FocusEvent::Lost).contains("Lost"));
}

// =============================================================================
// LineNumberMode
// =============================================================================

#[test]
fn line_number_mode_default() {
    let mode = LineNumberMode::default();
    assert_eq!(mode, LineNumberMode::None);
}

#[test]
fn line_number_mode_variants() {
    let modes = [
        LineNumberMode::None,
        LineNumberMode::Absolute,
        LineNumberMode::Relative,
        LineNumberMode::Hybrid,
    ];
    for (i, m) in modes.iter().enumerate() {
        for (j, other) in modes.iter().enumerate() {
            if i == j {
                assert_eq!(m, other);
            } else {
                assert_ne!(m, other);
            }
        }
    }
}

#[test]
fn line_number_mode_copy() {
    let a = LineNumberMode::Relative;
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// CursorInfo
// =============================================================================

#[test]
fn cursor_info_default() {
    let c = CursorInfo::default();
    assert_eq!(c.line, 0);
    assert_eq!(c.column, 0);
}

#[test]
fn cursor_info_construction() {
    let c = CursorInfo {
        line: 42,
        column: 10,
    };
    assert_eq!(c.line, 42);
    assert_eq!(c.column, 10);
}

#[test]
fn cursor_info_equality() {
    let a = CursorInfo { line: 1, column: 2 };
    let b = CursorInfo { line: 1, column: 2 };
    assert_eq!(a, b);
    let c = CursorInfo { line: 1, column: 3 };
    assert_ne!(a, c);
}

#[test]
fn cursor_info_copy() {
    let a = CursorInfo {
        line: 5,
        column: 10,
    };
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// SelectionMode
// =============================================================================

#[test]
fn selection_mode_variants() {
    let modes = [
        SelectionMode::Char,
        SelectionMode::Line,
        SelectionMode::Block,
    ];
    for (i, m) in modes.iter().enumerate() {
        for (j, other) in modes.iter().enumerate() {
            if i == j {
                assert_eq!(m, other);
            } else {
                assert_ne!(m, other);
            }
        }
    }
}

#[test]
fn selection_mode_copy() {
    let a = SelectionMode::Block;
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// SelectionInfo
// =============================================================================

#[test]
fn selection_info_construction() {
    let sel = SelectionInfo {
        start_line: 1,
        start_col: 0,
        end_line: 3,
        end_col: 5,
        mode: SelectionMode::Char,
        color: Color::Blue,
    };
    assert_eq!(sel.start_line, 1);
    assert_eq!(sel.start_col, 0);
    assert_eq!(sel.end_line, 3);
    assert_eq!(sel.end_col, 5);
    assert_eq!(sel.mode, SelectionMode::Char);
}

#[test]
fn selection_info_clone() {
    let sel = SelectionInfo {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 10,
        mode: SelectionMode::Line,
        color: Color::Red,
    };
    assert_eq!(Clone::clone(&sel).start_line, 0);
    assert_eq!(Clone::clone(&sel).end_col, 10);
    assert_eq!(Clone::clone(&sel).mode, SelectionMode::Line);
}

#[test]
fn selection_info_block_mode() {
    let sel = SelectionInfo {
        start_line: 5,
        start_col: 3,
        end_line: 10,
        end_col: 8,
        mode: SelectionMode::Block,
        color: Color::Rgb {
            r: 50,
            g: 50,
            b: 100,
        },
    };
    assert_eq!(sel.mode, SelectionMode::Block);
}

// =============================================================================
// RemoteClientInfo
// =============================================================================

#[test]
fn remote_client_info_construction() {
    let remote = RemoteClientInfo {
        client_id: 42,
        display_name: "Alice".to_string(),
        cursor_line: 10,
        cursor_col: 5,
        mode: "normal".to_string(),
        cursor_color: Color::Blue,
        selection: None,
    };
    assert_eq!(remote.client_id, 42);
    assert_eq!(remote.display_name, "Alice");
    assert_eq!(remote.cursor_line, 10);
    assert_eq!(remote.cursor_col, 5);
    assert_eq!(remote.mode, "normal");
    assert!(remote.selection.is_none());
}

#[test]
fn remote_client_info_with_selection() {
    let remote = RemoteClientInfo {
        client_id: 1,
        display_name: "Bob".to_string(),
        cursor_line: 5,
        cursor_col: 0,
        mode: "visual".to_string(),
        cursor_color: Color::Green,
        selection: Some(SelectionInfo {
            start_line: 5,
            start_col: 0,
            end_line: 8,
            end_col: 10,
            mode: SelectionMode::Char,
            color: Color::DarkBlue,
        }),
    };
    assert!(remote.selection.is_some());
    let sel = remote.selection.as_ref().unwrap();
    assert_eq!(sel.end_line, 8);
}

#[test]
fn remote_client_info_clone() {
    let remote = RemoteClientInfo {
        client_id: 99,
        display_name: "Clone".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        mode: "insert".to_string(),
        cursor_color: Color::Red,
        selection: None,
    };
    assert_eq!(Clone::clone(&remote).client_id, 99);
    assert_eq!(Clone::clone(&remote).display_name, "Clone");
}

// =============================================================================
// ViewportContext
// =============================================================================

#[test]
fn viewport_context_minimal() {
    let ctx = ViewportContext {
        buffer_id: None,
        buffer_lines: None,
        cursor: None,
        scroll_top: 0,
        local_selection: None,
        remote_clients: &[],
        fold_ranges: &[],
        virtual_lines: &[],
        opacity: 1.0,
        line_number_mode: LineNumberMode::None,
        gutter_width: 0,
        sidebar_width: 0,
        is_insert_mode: false,
        render_self_cursor: false,
        my_client_id: 0,
    };
    assert!(ctx.buffer_id.is_none());
    assert!(ctx.buffer_lines.is_none());
    assert!(ctx.cursor.is_none());
    assert!(ctx.local_selection.is_none());
    assert!(ctx.remote_clients.is_empty());
    assert!(ctx.fold_ranges.is_empty());
    assert!(ctx.virtual_lines.is_empty());
    assert!((ctx.opacity - 1.0).abs() < f32::EPSILON);
}

#[test]
fn viewport_context_with_content() {
    let lines = vec!["hello".to_string(), "world".to_string()];
    let remote = RemoteClientInfo {
        client_id: 1,
        display_name: "Peer".to_string(),
        cursor_line: 0,
        cursor_col: 3,
        mode: "normal".to_string(),
        cursor_color: Color::Blue,
        selection: None,
    };
    let remotes = vec![remote];
    let folds = vec![(5, 3)];

    let ctx = ViewportContext {
        buffer_id: Some(BufferId(1)),
        buffer_lines: Some(&lines),
        cursor: Some(CursorInfo { line: 0, column: 2 }),
        scroll_top: 0,
        local_selection: None,
        remote_clients: &remotes,
        fold_ranges: &folds,
        virtual_lines: &[],
        opacity: 0.8,
        line_number_mode: LineNumberMode::Absolute,
        gutter_width: 4,
        sidebar_width: 0,
        is_insert_mode: false,
        render_self_cursor: true,
        my_client_id: 42,
    };
    assert_eq!(ctx.buffer_id, Some(BufferId(1)));
    assert_eq!(ctx.buffer_lines.unwrap().len(), 2);
    assert_eq!(ctx.cursor.unwrap().column, 2);
    assert_eq!(ctx.remote_clients.len(), 1);
    assert_eq!(ctx.fold_ranges.len(), 1);
    assert_eq!(ctx.gutter_width, 4);
    assert!(ctx.render_self_cursor);
    assert_eq!(ctx.my_client_id, 42);
}

#[test]
fn viewport_context_debug() {
    let ctx = ViewportContext {
        buffer_id: None,
        buffer_lines: None,
        cursor: None,
        scroll_top: 0,
        local_selection: None,
        remote_clients: &[],
        fold_ranges: &[],
        virtual_lines: &[],
        opacity: 1.0,
        line_number_mode: LineNumberMode::None,
        gutter_width: 0,
        sidebar_width: 0,
        is_insert_mode: false,
        render_self_cursor: false,
        my_client_id: 0,
    };
    let debug = format!("{ctx:?}");
    assert!(debug.contains("ViewportContext"));
}

// =============================================================================
// PointerButton
// =============================================================================

#[test]
fn pointer_button_all_variants() {
    let buttons = [
        PointerButton::Left,
        PointerButton::Right,
        PointerButton::Middle,
    ];
    for b in buttons {
        assert!(!format!("{b:?}").is_empty());
    }
    assert_ne!(PointerButton::Left, PointerButton::Right);
}

// =============================================================================
// ClientModuleProbe
// =============================================================================

#[test]
fn client_module_probe_roundtrip() {
    let probe = ClientModuleProbe::new(
        "my-module",
        "My Module",
        Version::new(1, 2, 3),
        CLIENT_MODULE_API_VERSION,
    );
    assert_eq!(probe.id_str(), "my-module");
    assert_eq!(probe.name_str(), "My Module");
    assert_eq!(probe.version, Version::new(1, 2, 3));
    assert_eq!(probe.api_version, CLIENT_MODULE_API_VERSION);
}

#[test]
fn client_module_probe_deps() {
    let probe = ClientModuleProbe::new(
        "child",
        "Child Module",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    )
    .with_required_dep(0, "parent-a")
    .with_required_dep(1, "parent-b")
    .with_optional_dep(0, "optional-x");

    let required = probe.required_deps();
    assert_eq!(required.len(), 2);
    assert_eq!(required[0], "parent-a");
    assert_eq!(required[1], "parent-b");

    let optional = probe.optional_deps();
    assert_eq!(optional.len(), 1);
    assert_eq!(optional[0], "optional-x");
}

#[test]
fn client_module_probe_empty_deps() {
    let probe = ClientModuleProbe::new(
        "standalone",
        "Standalone",
        Version::new(0, 1, 0),
        CLIENT_MODULE_API_VERSION,
    );
    assert!(probe.required_deps().is_empty());
    assert!(probe.optional_deps().is_empty());
}

#[test]
fn client_module_probe_copy_semantics() {
    let a = ClientModuleProbe::new(
        "copy-test",
        "Copy Test",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    );
    let b = a;
    assert_eq!(a.id_str(), b.id_str());
    assert_eq!(a.name_str(), b.name_str());
}

// =============================================================================
// ClientModuleProbe capabilities
// =============================================================================

#[test]
fn client_module_probe_default_capabilities_all_false() {
    let probe = ClientModuleProbe::new(
        "cap-test",
        "Cap Test",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    );
    assert!(!probe.has_chrome());
    assert!(!probe.has_buffer_contrib());
    assert!(!probe.has_annotations());
    assert_eq!(probe.capabilities, 0);
}

#[test]
fn client_module_probe_capabilities_all_set() {
    let probe = ClientModuleProbe::new(
        "cap-test",
        "Cap Test",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    )
    .with_capabilities(true, true, true);
    assert!(probe.has_chrome());
    assert!(probe.has_buffer_contrib());
    assert!(probe.has_annotations());
    assert_eq!(probe.capabilities, 0b111);
}

#[test]
fn client_module_probe_capabilities_individual() {
    let chrome_only =
        ClientModuleProbe::new("c", "C", Version::new(1, 0, 0), CLIENT_MODULE_API_VERSION)
            .with_capabilities(true, false, false);
    assert!(chrome_only.has_chrome());
    assert!(!chrome_only.has_buffer_contrib());
    assert!(!chrome_only.has_annotations());

    let buffer_only =
        ClientModuleProbe::new("b", "B", Version::new(1, 0, 0), CLIENT_MODULE_API_VERSION)
            .with_capabilities(false, true, false);
    assert!(!buffer_only.has_chrome());
    assert!(buffer_only.has_buffer_contrib());
    assert!(!buffer_only.has_annotations());

    let annotation_only =
        ClientModuleProbe::new("a", "A", Version::new(1, 0, 0), CLIENT_MODULE_API_VERSION)
            .with_capabilities(false, false, true);
    assert!(!annotation_only.has_chrome());
    assert!(!annotation_only.has_buffer_contrib());
    assert!(annotation_only.has_annotations());
}

#[test]
fn client_module_probe_capabilities_roundtrip_via_copy() {
    let a =
        ClientModuleProbe::new("rt", "Roundtrip", Version::new(1, 0, 0), CLIENT_MODULE_API_VERSION)
            .with_capabilities(true, false, true);
    let b = a;
    assert!(b.has_chrome());
    assert!(!b.has_buffer_contrib());
    assert!(b.has_annotations());
}

// =============================================================================
// CLIENT_MODULE_API_VERSION
// =============================================================================

#[test]
fn client_api_version_exists() {
    let v = CLIENT_MODULE_API_VERSION;
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 3);
    assert_eq!(v.patch, 0);
}

// =============================================================================
// is_client_compatible
// =============================================================================

#[test]
fn is_client_compatible_same_version() {
    assert!(is_client_compatible(Version::new(1, 0, 0), Version::new(1, 0, 0)));
}

#[test]
fn is_client_compatible_same_major_higher_minor() {
    assert!(is_client_compatible(Version::new(1, 0, 0), Version::new(1, 2, 0)));
}

#[test]
fn is_client_compatible_same_major_lower_minor() {
    assert!(!is_client_compatible(Version::new(1, 3, 0), Version::new(1, 2, 0)));
}

#[test]
fn is_client_compatible_different_major() {
    assert!(!is_client_compatible(Version::new(2, 0, 0), Version::new(1, 0, 0)));
}

#[test]
fn is_client_compatible_zero_major() {
    assert!(is_client_compatible(Version::new(0, 1, 0), Version::new(0, 1, 0)));
    assert!(!is_client_compatible(Version::new(0, 1, 0), Version::new(0, 0, 0)));
}

#[test]
fn is_client_compatible_v01_module_against_v02_loader() {
    // 0.1.x module (requires 0.1.0) is compatible with 0.2.0 loader
    assert!(is_client_compatible(Version::new(0, 1, 0), Version::new(0, 2, 0)));
}

#[test]
fn is_client_compatible_v02_module_against_v01_loader() {
    // 0.2.x module (requires 0.2.0) is NOT compatible with 0.1.0 loader
    assert!(!is_client_compatible(Version::new(0, 2, 0), Version::new(0, 1, 0)));
}

// =============================================================================
// Rect::intersect
// =============================================================================

#[test]
fn rect_intersect_overlapping() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(5, 5, 10, 10);
    let result = a.intersect(&b).unwrap();
    assert_eq!(result, Rect::new(5, 5, 5, 5));
}

#[test]
fn rect_intersect_no_overlap_x() {
    // x1 >= x2 (no horizontal overlap)
    let a = Rect::new(0, 0, 5, 10);
    let b = Rect::new(5, 0, 5, 10);
    assert!(a.intersect(&b).is_none());
}

#[test]
fn rect_intersect_no_overlap_y() {
    // y1 >= y2 (no vertical overlap)
    let a = Rect::new(0, 0, 10, 5);
    let b = Rect::new(0, 5, 10, 5);
    assert!(a.intersect(&b).is_none());
}

#[test]
fn rect_intersect_contained() {
    let outer = Rect::new(0, 0, 20, 20);
    let inner = Rect::new(5, 5, 5, 5);
    let result = outer.intersect(&inner).unwrap();
    assert_eq!(result, inner);
}

#[test]
fn rect_intersect_same() {
    let r = Rect::new(3, 3, 10, 10);
    let result = r.intersect(&r).unwrap();
    assert_eq!(result, r);
}

#[test]
fn rect_intersect_no_overlap_both() {
    // Both x and y don't overlap
    let a = Rect::new(0, 0, 5, 5);
    let b = Rect::new(10, 10, 5, 5);
    assert!(a.intersect(&b).is_none());
}

// =============================================================================
// Rect::contains_point
// =============================================================================

#[test]
fn rect_contains_point_inside() {
    let r = Rect::new(5, 5, 10, 10);
    assert!(r.contains_point(5, 5));   // top-left corner
    assert!(r.contains_point(10, 10)); // interior
    assert!(r.contains_point(14, 14)); // just inside
}

#[test]
fn rect_contains_point_outside_left() {
    let r = Rect::new(5, 5, 10, 10);
    assert!(!r.contains_point(4, 10)); // x < self.x
}

#[test]
fn rect_contains_point_outside_above() {
    let r = Rect::new(5, 5, 10, 10);
    assert!(!r.contains_point(10, 4)); // y < self.y
}

#[test]
fn rect_contains_point_outside_right() {
    let r = Rect::new(5, 5, 10, 10);
    assert!(!r.contains_point(15, 10)); // x >= self.x + self.width
}

#[test]
fn rect_contains_point_outside_below() {
    let r = Rect::new(5, 5, 10, 10);
    assert!(!r.contains_point(10, 15)); // y >= self.y + self.height
}

#[test]
fn rect_contains_point_at_origin() {
    let r = Rect::new(0, 0, 10, 10);
    assert!(r.contains_point(0, 0));
    assert!(!r.contains_point(10, 0));
    assert!(!r.contains_point(0, 10));
}

// =============================================================================
// Style builder methods
// =============================================================================

#[test]
fn style_builder_fg() {
    let s = Style::new().fg(Color::Red);
    assert_eq!(s.fg, Some(Color::Red));
    assert!(s.bg.is_none());
}

#[test]
fn style_builder_bg() {
    let s = Style::new().bg(Color::Blue);
    assert!(s.fg.is_none());
    assert_eq!(s.bg, Some(Color::Blue));
}

#[test]
fn style_builder_bold() {
    let s = Style::new().bold();
    assert!(s.attributes.contains(Attributes::BOLD));
}

#[test]
fn style_builder_italic() {
    let s = Style::new().italic();
    assert!(s.attributes.contains(Attributes::ITALIC));
}

#[test]
fn style_builder_underline() {
    let s = Style::new().underline();
    assert!(s.attributes.contains(Attributes::UNDERLINE));
}

#[test]
fn style_builder_dim() {
    let s = Style::new().dim();
    assert!(s.attributes.contains(Attributes::DIM));
}

#[test]
fn style_builder_reverse() {
    let s = Style::new().reverse();
    assert!(s.attributes.contains(Attributes::REVERSE));
}

#[test]
fn style_builder_chained() {
    let s = Style::new()
        .fg(Color::Red)
        .bg(Color::Blue)
        .bold()
        .italic()
        .underline()
        .dim()
        .reverse();
    assert_eq!(s.fg, Some(Color::Red));
    assert_eq!(s.bg, Some(Color::Blue));
    assert!(s.attributes.contains(Attributes::BOLD));
    assert!(s.attributes.contains(Attributes::ITALIC));
    assert!(s.attributes.contains(Attributes::UNDERLINE));
    assert!(s.attributes.contains(Attributes::DIM));
    assert!(s.attributes.contains(Attributes::REVERSE));
}

// =============================================================================
// ClientModuleProbe field truncation
// =============================================================================

#[test]
fn client_module_probe_id_truncated_at_64() {
    let long_id = "a".repeat(100);
    let probe = ClientModuleProbe::new(
        &long_id,
        "name",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    );
    let id = probe.id_str();
    assert_eq!(id.len(), 64);
}

#[test]
fn client_module_probe_name_truncated_at_128() {
    let long_name = "b".repeat(200);
    let probe = ClientModuleProbe::new(
        "id",
        &long_name,
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    );
    let name = probe.name_str();
    assert_eq!(name.len(), 128);
}

#[test]
fn client_module_probe_with_required_dep_index_out_of_range() {
    let probe = ClientModuleProbe::new(
        "id",
        "name",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    )
    .with_required_dep(8, "dep");
    assert!(probe.required_deps().is_empty());
}

#[test]
fn client_module_probe_with_optional_dep_index_out_of_range() {
    let probe = ClientModuleProbe::new(
        "id",
        "name",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    )
    .with_optional_dep(8, "dep");
    assert!(probe.optional_deps().is_empty());
}

#[test]
fn client_module_probe_with_required_dep_long_name() {
    let long_dep = "x".repeat(100);
    let probe = ClientModuleProbe::new(
        "id",
        "name",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    )
    .with_required_dep(0, &long_dep);
    let deps = probe.required_deps();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].len(), 64);
}

#[test]
fn client_module_probe_with_optional_dep_long_name() {
    let long_dep = "y".repeat(100);
    let probe = ClientModuleProbe::new(
        "id",
        "name",
        Version::new(1, 0, 0),
        CLIENT_MODULE_API_VERSION,
    )
    .with_optional_dep(0, &long_dep);
    let deps = probe.optional_deps();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].len(), 64);
}
