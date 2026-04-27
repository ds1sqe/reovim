//! Tests for TUI codec encode/decode roundtrips and error cases.

use reovim_input_codec::InputPayloadError;

use {
    crate::{
        KeyCode, KeyEvent, KeyEventKind, Modifiers, MouseButton, MouseEvent, MouseEventKind,
        ScrollEvent,
        codecs::{
            KIND_KEY, KIND_MOUSE, KIND_SCROLL, TuiKeyCodec, TuiMouseCodec, TuiScrollCodec,
            decode_key_event, decode_mouse_event, decode_scroll_event, encode_key_event,
            encode_mouse_event, encode_scroll_event,
        },
        key_types::keycode_to_u32,
    },
    reovim_input_codec::Codec,
};

// ---------------------------------------------------------------------------
// TuiKeyCodec
// ---------------------------------------------------------------------------

fn all_key_events() -> Vec<KeyEvent> {
    let codes = [
        KeyCode::Null,
        KeyCode::Backspace,
        KeyCode::Enter,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Delete,
        KeyCode::Insert,
        KeyCode::Escape,
        KeyCode::CapsLock,
        KeyCode::ScrollLock,
        KeyCode::NumLock,
        KeyCode::PrintScreen,
        KeyCode::Pause,
        KeyCode::Menu,
        KeyCode::KeypadBegin,
        KeyCode::MediaPlay,
        KeyCode::MediaPause,
        KeyCode::MediaPlayPause,
        KeyCode::MediaStop,
        KeyCode::MediaReverse,
        KeyCode::MediaFastForward,
        KeyCode::MediaRewind,
        KeyCode::MediaNext,
        KeyCode::MediaPrevious,
        KeyCode::MediaRecord,
        KeyCode::MediaLowerVolume,
        KeyCode::MediaRaiseVolume,
        KeyCode::MediaMuteVolume,
        KeyCode::LeftShift,
        KeyCode::RightShift,
        KeyCode::LeftCtrl,
        KeyCode::RightCtrl,
        KeyCode::LeftAlt,
        KeyCode::RightAlt,
        KeyCode::LeftSuper,
        KeyCode::RightSuper,
        KeyCode::LeftHyper,
        KeyCode::RightHyper,
        KeyCode::LeftMeta,
        KeyCode::RightMeta,
        KeyCode::IsoLevel3Shift,
        KeyCode::IsoLevel5Shift,
        KeyCode::Char('a'),
        KeyCode::Char('Z'),
        KeyCode::Char(' '),
        KeyCode::Char('\u{1F600}'),
    ];
    let mut events: Vec<KeyEvent> = vec![];
    for code in &codes {
        events.push(KeyEvent::new(*code));
    }
    // F1-F24
    for n in 1u8..=24 {
        events.push(KeyEvent::new(KeyCode::F(n)));
    }
    events
}

#[test]
fn tui_key_codec_roundtrip_all_variants() {
    let codec = TuiKeyCodec;
    for event in all_key_events() {
        let body = codec.encode(&event).expect("encode failed");
        let decoded = codec.decode(&body).expect("decode failed");
        let recovered = decoded
            .downcast::<KeyEvent>()
            .expect("downcast to KeyEvent failed");
        // Body-only roundtrip preserves keycode; modifiers/kind come from header.
        assert_eq!(
            keycode_to_u32(&recovered.code),
            keycode_to_u32(&event.code),
            "keycode mismatch for {event:?}"
        );
    }
}

#[test]
fn tui_key_codec_full_payload_roundtrip() {
    let event = KeyEvent::full(
        KeyCode::Char('w'),
        Modifiers::CTRL | Modifiers::SHIFT,
        KeyEventKind::Repeat,
    );
    let payload = encode_key_event(&event);
    assert_eq!(payload.len(), 12); // 8 header + 4 body
    let recovered = decode_key_event(&payload).expect("decode failed");
    assert_eq!(recovered.code, KeyCode::Char('w'));
    assert_eq!(recovered.modifiers, Modifiers::CTRL | Modifiers::SHIFT);
    assert_eq!(recovered.kind, KeyEventKind::Repeat);
}

#[test]
fn tui_key_codec_kind_value() {
    assert_eq!(TuiKeyCodec.kind(), KIND_KEY);
    assert_eq!(KIND_KEY, 0x0001);
}

#[test]
fn tui_key_codec_decode_truncated_body_returns_too_short() {
    let codec = TuiKeyCodec;
    let err = codec.decode(&[0x01, 0x02]).unwrap_err();
    assert!(matches!(err, InputPayloadError::TooShort { .. }));
}

#[test]
fn tui_key_codec_decode_truncated_full_payload_returns_too_short() {
    let err = decode_key_event(&[0u8; 7]).unwrap_err();
    assert!(matches!(err, InputPayloadError::TooShort { .. }));
}

#[test]
fn tui_key_codec_wrong_type_returns_error() {
    let codec = TuiKeyCodec;
    let not_a_key: u32 = 42;
    let err = codec.encode(&not_a_key).unwrap_err();
    assert!(matches!(
        err,
        InputPayloadError::WrongType {
            expected: "KeyEvent"
        }
    ));
}

#[test]
fn tui_key_codec_all_kinds() {
    for (kind_variant, code) in [
        (KeyEventKind::Press, KeyCode::Char('a')),
        (KeyEventKind::Release, KeyCode::Escape),
        (KeyEventKind::Repeat, KeyCode::Enter),
    ] {
        let event = KeyEvent::full(code, Modifiers::NONE, kind_variant);
        let payload = encode_key_event(&event);
        let recovered = decode_key_event(&payload).unwrap();
        assert_eq!(recovered.kind, kind_variant);
        assert_eq!(recovered.code, code);
    }
}

// ---------------------------------------------------------------------------
// TuiMouseCodec
// ---------------------------------------------------------------------------

fn all_mouse_events() -> Vec<MouseEvent> {
    use MouseEventKind::*;
    let kinds = [
        Moved,
        Down(MouseButton::Left),
        Down(MouseButton::Right),
        Down(MouseButton::Middle),
        Up(MouseButton::Left),
        Up(MouseButton::Right),
        Up(MouseButton::Middle),
        Drag(MouseButton::Left),
        Drag(MouseButton::Right),
        Drag(MouseButton::Middle),
        ScrollUp,
        ScrollDown,
        ScrollLeft,
        ScrollRight,
    ];
    kinds.iter().map(|k| MouseEvent::new(*k, 10, 20)).collect()
}

#[test]
fn tui_mouse_codec_roundtrip_all_variants() {
    let codec = TuiMouseCodec;
    for event in all_mouse_events() {
        let body = codec.encode(&event).expect("encode failed");
        let decoded = codec.decode(&body).expect("decode failed");
        let recovered = decoded
            .downcast::<MouseEvent>()
            .expect("downcast to MouseEvent failed");
        assert_eq!(recovered.kind, event.kind, "kind mismatch for {event:?}");
        assert_eq!(recovered.column, event.column);
        assert_eq!(recovered.row, event.row);
    }
}

#[test]
fn tui_mouse_codec_full_payload_roundtrip() {
    let event = MouseEvent::with_modifiers(
        MouseEventKind::Down(MouseButton::Middle),
        42,
        17,
        Modifiers::ALT,
    );
    let payload = encode_mouse_event(&event);
    assert_eq!(payload.len(), 14); // 8 header + 6 body
    let recovered = decode_mouse_event(&payload).unwrap();
    assert_eq!(recovered.kind, event.kind);
    assert_eq!(recovered.column, 42);
    assert_eq!(recovered.row, 17);
    assert_eq!(recovered.modifiers, Modifiers::ALT);
}

#[test]
fn tui_mouse_codec_kind_value() {
    assert_eq!(TuiMouseCodec.kind(), KIND_MOUSE);
    assert_eq!(KIND_MOUSE, 0x0002);
}

#[test]
fn tui_mouse_codec_decode_truncated_body_returns_too_short() {
    let codec = TuiMouseCodec;
    let err = codec.decode(&[0x01]).unwrap_err();
    assert!(matches!(err, InputPayloadError::TooShort { .. }));
}

#[test]
fn tui_mouse_codec_decode_truncated_full_payload_returns_too_short() {
    let err = decode_mouse_event(&[0u8; 10]).unwrap_err();
    assert!(matches!(err, InputPayloadError::TooShort { .. }));
}

#[test]
fn tui_mouse_codec_wrong_type_returns_error() {
    let codec = TuiMouseCodec;
    let not_a_mouse: u32 = 99;
    let err = codec.encode(&not_a_mouse).unwrap_err();
    assert!(matches!(
        err,
        InputPayloadError::WrongType {
            expected: "MouseEvent"
        }
    ));
}

// ---------------------------------------------------------------------------
// TuiScrollCodec
// ---------------------------------------------------------------------------

#[test]
fn tui_scroll_codec_roundtrip() {
    let codec = TuiScrollCodec;
    let event = ScrollEvent::new(-3, 7, 15, 22, Modifiers::CTRL);
    let body = codec.encode(&event).expect("encode failed");
    let decoded = codec.decode(&body).expect("decode failed");
    let recovered = decoded
        .downcast::<ScrollEvent>()
        .expect("downcast to ScrollEvent failed");
    // Body-only: dx/dy/x/y preserved; modifiers come from header.
    assert_eq!(recovered.dx, -3);
    assert_eq!(recovered.dy, 7);
    assert_eq!(recovered.x, 15);
    assert_eq!(recovered.y, 22);
}

#[test]
fn tui_scroll_codec_full_payload_roundtrip() {
    let event = ScrollEvent::new(1, -2, 5, 10, Modifiers::SHIFT);
    let payload = encode_scroll_event(&event);
    assert_eq!(payload.len(), 18); // 8 header + 10 body
    let recovered = decode_scroll_event(&payload).unwrap();
    assert_eq!(recovered.dx, 1);
    assert_eq!(recovered.dy, -2);
    assert_eq!(recovered.x, 5);
    assert_eq!(recovered.y, 10);
    assert_eq!(recovered.modifiers, Modifiers::SHIFT);
}

#[test]
fn tui_scroll_codec_vertical_helper() {
    let event = ScrollEvent::vertical(-1);
    let payload = encode_scroll_event(&event);
    let recovered = decode_scroll_event(&payload).unwrap();
    assert_eq!(recovered.dy, -1);
    assert_eq!(recovered.dx, 0);
}

#[test]
fn tui_scroll_codec_kind_value() {
    assert_eq!(TuiScrollCodec.kind(), KIND_SCROLL);
    assert_eq!(KIND_SCROLL, 0x0003);
}

#[test]
fn tui_scroll_codec_decode_truncated_body_returns_too_short() {
    let codec = TuiScrollCodec;
    let err = codec.decode(&[0x01, 0x02, 0x03]).unwrap_err();
    assert!(matches!(err, InputPayloadError::TooShort { .. }));
}

#[test]
fn tui_scroll_codec_decode_truncated_full_payload_returns_too_short() {
    let err = decode_scroll_event(&[0u8; 12]).unwrap_err();
    assert!(matches!(err, InputPayloadError::TooShort { .. }));
}

#[test]
fn tui_scroll_codec_wrong_type_returns_error() {
    let codec = TuiScrollCodec;
    let not_a_scroll: bool = true;
    let err = codec.encode(&not_a_scroll).unwrap_err();
    assert!(matches!(
        err,
        InputPayloadError::WrongType {
            expected: "ScrollEvent"
        }
    ));
}

// ---------------------------------------------------------------------------
// Registry integration (codec objects in Arc<dyn Codec>)
// ---------------------------------------------------------------------------

#[test]
fn codecs_arc_dyn_codec_roundtrip() {
    use {
        crate::codecs::{tui_key_codec, tui_mouse_codec, tui_scroll_codec},
        reovim_input_codec::Codec,
        std::sync::Arc,
    };

    let codecs: Vec<Arc<dyn Codec>> = vec![tui_key_codec(), tui_mouse_codec(), tui_scroll_codec()];
    let kinds: Vec<u16> = codecs.iter().map(|c| c.kind()).collect();
    assert_eq!(kinds, vec![KIND_KEY, KIND_MOUSE, KIND_SCROLL]);
}

#[test]
fn tui_key_codec_register_unregister_via_registry() {
    use {
        crate::codecs::{
            KIND_KEY, KIND_MOUSE, KIND_SCROLL, tui_key_codec, tui_mouse_codec, tui_scroll_codec,
        },
        reovim_subsys_input::{DefaultInputCodecRegistry, InputCodecRegistry},
    };

    let registry = DefaultInputCodecRegistry::new();
    registry.register(tui_key_codec());
    registry.register(tui_mouse_codec());
    registry.register(tui_scroll_codec());

    assert!(registry.get(KIND_KEY).is_some());
    assert!(registry.get(KIND_MOUSE).is_some());
    assert!(registry.get(KIND_SCROLL).is_some());

    registry.unregister(KIND_KEY);
    registry.unregister(KIND_MOUSE);
    registry.unregister(KIND_SCROLL);

    assert!(registry.get(KIND_KEY).is_none());
    assert!(registry.get(KIND_MOUSE).is_none());
    assert!(registry.get(KIND_SCROLL).is_none());
}
