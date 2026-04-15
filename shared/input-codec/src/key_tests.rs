//! Round-trip tests for keyboard codec.

use crate::key::{decode, encode, KIND_KEY};
use reovim_subsys_input::{input_event::INPUT_HEADER_SIZE, input_kind, KeyCode, KeyEvent, KeyEventKind, Modifiers};

#[test]
fn round_trip_char_press() {
    let event = KeyEvent {
        code: KeyCode::Char('a'),
        modifiers: Modifiers::NONE,
        kind: KeyEventKind::Press,
    };
    let payload = encode(&event);
    assert_eq!(input_kind(&payload), KIND_KEY);
    assert!(payload.len() >= INPUT_HEADER_SIZE + 4);

    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.code, KeyCode::Char('a'));
    assert_eq!(decoded.modifiers, Modifiers::NONE);
    assert_eq!(decoded.kind, KeyEventKind::Press);
}

#[test]
fn round_trip_ctrl_shift() {
    let event = KeyEvent {
        code: KeyCode::Char('x'),
        modifiers: Modifiers::CTRL | Modifiers::SHIFT,
        kind: KeyEventKind::Press,
    };
    let payload = encode(&event);
    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.code, KeyCode::Char('x'));
    assert_eq!(decoded.modifiers, Modifiers::CTRL | Modifiers::SHIFT);
}

#[test]
fn round_trip_function_key() {
    let event = KeyEvent {
        code: KeyCode::F(12),
        modifiers: Modifiers::ALT,
        kind: KeyEventKind::Release,
    };
    let payload = encode(&event);
    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.code, KeyCode::F(12));
    assert_eq!(decoded.modifiers, Modifiers::ALT);
    assert_eq!(decoded.kind, KeyEventKind::Release);
}

#[test]
fn round_trip_special_keys() {
    for code in [
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
        KeyCode::Null,
    ] {
        let event = KeyEvent {
            code,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        };
        let payload = encode(&event);
        let decoded = decode(&payload).unwrap();
        assert_eq!(decoded.code, code, "failed round-trip for {code:?}");
    }
}

#[test]
fn round_trip_repeat() {
    let event = KeyEvent {
        code: KeyCode::Char('j'),
        modifiers: Modifiers::NONE,
        kind: KeyEventKind::Repeat,
    };
    let payload = encode(&event);
    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.kind, KeyEventKind::Repeat);
}

#[test]
fn decode_wrong_kind_returns_none() {
    let mut payload = vec![0u8; INPUT_HEADER_SIZE + 4];
    payload[0] = 0x02; // KIND_POINTER, not KIND_KEY
    assert!(decode(&payload).is_none());
}

#[test]
fn decode_short_payload_returns_none() {
    let payload = vec![0u8; INPUT_HEADER_SIZE]; // no body
    assert!(decode(&payload).is_none());
}

#[test]
fn round_trip_unicode_char() {
    let event = KeyEvent {
        code: KeyCode::Char('한'),
        modifiers: Modifiers::NONE,
        kind: KeyEventKind::Press,
    };
    let payload = encode(&event);
    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.code, KeyCode::Char('한'));
}
