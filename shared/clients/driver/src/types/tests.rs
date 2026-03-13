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
    let err = ClientModuleError {
        message: "boom".to_string(),
    };
    let result = ProbeResult::Failed(err);
    assert!(matches!(result, ProbeResult::Failed(ref e) if e.message == "boom"));
}

// =============================================================================
// ClientModuleError
// =============================================================================

#[test]
fn client_module_error_construction() {
    let err = ClientModuleError {
        message: "test error".to_string(),
    };
    assert_eq!(err.message, "test error");
}

#[test]
fn client_module_error_empty_message() {
    let err = ClientModuleError {
        message: String::new(),
    };
    assert!(err.message.is_empty());
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
// PointerButton
// =============================================================================

#[test]
fn pointer_button_all_variants() {
    let buttons = [PointerButton::Left, PointerButton::Right, PointerButton::Middle];
    for b in buttons {
        assert!(!format!("{b:?}").is_empty());
    }
    assert_ne!(PointerButton::Left, PointerButton::Right);
}
