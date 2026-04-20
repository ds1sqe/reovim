//! Tests for TUI key types — `KeyCode`, `Modifiers`, `KeyEvent`, `KeymapResult`.

use crate::{
    KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers,
    key_types::{keycode_to_u32, u32_to_keycode},
};

// ---------------------------------------------------------------------------
// KeyCode roundtrip — every variant
// ---------------------------------------------------------------------------

fn all_keycodes() -> Vec<KeyCode> {
    let mut codes = vec![
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
        // Media keys
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
        // Per-side modifier keys
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
    ];
    // Char variants (sample of printable ASCII + Unicode)
    codes.extend([
        KeyCode::Char('a'),
        KeyCode::Char('Z'),
        KeyCode::Char('0'),
        KeyCode::Char(' '),
        KeyCode::Char('\t'),
        KeyCode::Char('\u{1F600}'), // emoji
    ]);
    // Function keys F1-F24
    for n in 1u8..=24 {
        codes.push(KeyCode::F(n));
    }
    codes
}

#[test]
fn keycode_u32_roundtrip_all_variants() {
    for code in all_keycodes() {
        let encoded = keycode_to_u32(&code);
        let decoded = u32_to_keycode(encoded);
        assert_eq!(
            decoded, code,
            "roundtrip failed for {code:?}: encoded={encoded:#010x}"
        );
    }
}

#[test]
fn unknown_u32_decodes_to_null() {
    // 0x80 is in the reserved "no flag bits set, no named key" range → Null.
    let decoded = u32_to_keycode(0x0000_0080);
    assert_eq!(decoded, KeyCode::Null);
    // Also test 0x0050 (another gap value).
    let decoded2 = u32_to_keycode(0x0000_0050);
    assert_eq!(decoded2, KeyCode::Null);
}

// ---------------------------------------------------------------------------
// Modifiers
// ---------------------------------------------------------------------------

#[test]
fn modifiers_bitwise_combinations() {
    let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
    assert!(ctrl_shift.contains(Modifiers::CTRL));
    assert!(ctrl_shift.contains(Modifiers::SHIFT));
    assert!(!ctrl_shift.contains(Modifiers::ALT));
    assert!(!ctrl_shift.contains(Modifiers::SUPER));
    assert!(!ctrl_shift.contains(Modifiers::HYPER));
    assert!(!ctrl_shift.contains(Modifiers::META));
}

#[test]
fn modifiers_all_distinct_flags() {
    // Each modifier occupies a unique bit.
    let all = [
        Modifiers::SHIFT,
        Modifiers::CTRL,
        Modifiers::ALT,
        Modifiers::SUPER,
        Modifiers::HYPER,
        Modifiers::META,
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i != j {
                assert!(
                    (*a & *b).is_empty(),
                    "Modifiers::{a:?} and Modifiers::{b:?} share a bit"
                );
            }
        }
    }
}

#[test]
fn modifiers_default_is_none() {
    assert_eq!(Modifiers::default(), Modifiers::NONE);
    assert!(Modifiers::NONE.is_empty());
}

// ---------------------------------------------------------------------------
// KeyEvent
// ---------------------------------------------------------------------------

#[test]
fn key_event_constructors() {
    let press = KeyEvent::new(KeyCode::Enter);
    assert_eq!(press.code, KeyCode::Enter);
    assert_eq!(press.modifiers, Modifiers::NONE);
    assert!(press.is_press());
    assert!(!press.is_release());
    assert!(!press.is_repeat());

    let with_mod = KeyEvent::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL);
    assert_eq!(with_mod.code, KeyCode::Char('w'));
    assert_eq!(with_mod.modifiers, Modifiers::CTRL);
    assert!(with_mod.is_press());

    let release = KeyEvent::full(KeyCode::Escape, Modifiers::SHIFT, KeyEventKind::Release);
    assert!(release.is_release());

    let repeat_ev = KeyEvent::full(KeyCode::Char('j'), Modifiers::NONE, KeyEventKind::Repeat);
    assert!(repeat_ev.is_repeat());
}

#[test]
fn key_event_hash_eq_used_in_collections() {
    use std::collections::{HashMap, HashSet};

    let a = KeyEvent::new(KeyCode::Char('g'));
    let b = KeyEvent::new(KeyCode::Char('g'));
    let c = KeyEvent::new(KeyCode::Char('d'));

    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
    assert!(!set.contains(&c));

    let mut map: HashMap<KeyEvent, u32> = HashMap::new();
    map.insert(a, 42);
    assert_eq!(map[&b], 42);
}

// ---------------------------------------------------------------------------
// KeymapResult
// ---------------------------------------------------------------------------

#[test]
fn keymap_result_match_helpers() {
    let m: KeymapResult<&str> = KeymapResult::Match("action");
    assert!(m.is_match());
    assert!(!m.is_prefix());
    assert!(!m.is_none());
    assert_eq!(m.unwrap(), "action");
}

#[test]
fn keymap_result_prefix_helpers() {
    let p: KeymapResult<u32> = KeymapResult::Prefix;
    assert!(!p.is_match());
    assert!(p.is_prefix());
    assert!(!p.is_none());
    assert_eq!(p.into_option(), None);
}

#[test]
fn keymap_result_none_helpers() {
    let n: KeymapResult<u32> = KeymapResult::None;
    assert!(!n.is_match());
    assert!(!n.is_prefix());
    assert!(n.is_none());
    assert_eq!(n.into_option(), None);
}

#[test]
fn keymap_result_map_transforms_action() {
    let m: KeymapResult<u32> = KeymapResult::Match(10);
    let mapped = m.map(|v| v * 2);
    assert_eq!(mapped.unwrap(), 20);

    let p: KeymapResult<u32> = KeymapResult::Prefix;
    let mapped_p = p.map(|v: u32| v * 2);
    assert!(mapped_p.is_prefix());

    let n: KeymapResult<u32> = KeymapResult::None;
    let mapped_n = n.map(|v: u32| v * 2);
    assert!(mapped_n.is_none());
}

#[test]
#[should_panic(expected = "Prefix")]
fn keymap_result_unwrap_prefix_panics() {
    let p: KeymapResult<u32> = KeymapResult::Prefix;
    let _ = p.unwrap();
}

#[test]
#[should_panic(expected = "None")]
fn keymap_result_unwrap_none_panics() {
    let n: KeymapResult<u32> = KeymapResult::None;
    let _ = n.unwrap();
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn key_event_debug_mentions_type() {
    let dbg = format!("{:?}", KeyEvent::new(KeyCode::Enter));
    assert!(dbg.contains("KeyEvent"));
}
