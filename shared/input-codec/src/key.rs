//! Keyboard input codec — encode/decode `PlatformEvent::Key` into `InputEvent` payload.
//!
//! Kind: `KIND_KEY = 0x0001`
//!
//! Body layout (after 8-byte header):
//! - bytes 0-3: keycode (u32, little-endian)
//!
//! Context bits 0-7: modifier flags (Shift/Ctrl/Alt/Super/Meta)
//! Flags field: PRESS, RELEASE, REPEAT from InputFlags

use reovim_arch::KeyCode as ArchKeyCode;
use reovim_subsys_input::{
    input_event::INPUT_HEADER_SIZE, InputFlags, KeyCode, KeyEvent, KeyEventKind, Modifiers,
};

/// Well-known kind for keyboard input.
pub const KIND_KEY: u16 = 0x0001;

/// Encode a `KeyEvent` into an `InputEvent` payload (header + body).
pub fn encode(event: &KeyEvent) -> Vec<u8> {
    let flags = match event.kind {
        KeyEventKind::Press => InputFlags::PRESS,
        KeyEventKind::Repeat => InputFlags::REPEAT,
        KeyEventKind::Release => InputFlags::RELEASE,
    };

    let context = u32::from(event.modifiers.bits());
    let keycode_u32 = keycode_to_u32(&event.code);

    let mut buf = Vec::with_capacity(INPUT_HEADER_SIZE + 4);
    buf.extend(KIND_KEY.to_le_bytes());
    buf.extend(flags.bits().to_le_bytes());
    buf.extend(context.to_le_bytes());
    buf.extend(keycode_u32.to_le_bytes());
    buf
}

/// Decode a `KeyEvent` from an `InputEvent` payload.
///
/// Returns `None` if the payload is too short or the kind doesn't match.
pub fn decode(payload: &[u8]) -> Option<KeyEvent> {
    if payload.len() < INPUT_HEADER_SIZE + 4 {
        return None;
    }

    let kind = u16::from_le_bytes([payload[0], payload[1]]);
    if kind != KIND_KEY {
        return None;
    }

    let flags =
        InputFlags::from_bits_truncate(u16::from_le_bytes([payload[2], payload[3]]));
    let context = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let keycode_u32 = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);

    let event_kind = if flags.contains(InputFlags::PRESS) {
        KeyEventKind::Press
    } else if flags.contains(InputFlags::REPEAT) {
        KeyEventKind::Repeat
    } else {
        KeyEventKind::Release
    };

    #[allow(clippy::cast_possible_truncation)]
    let modifiers = Modifiers::from_bits_truncate(context as u8);
    let code = u32_to_keycode(keycode_u32);

    Some(KeyEvent {
        code,
        modifiers,
        kind: event_kind,
    })
}

fn keycode_to_u32(code: &KeyCode) -> u32 {
    match code {
        KeyCode::Null => 0x0000,
        KeyCode::Backspace => 0x0001,
        KeyCode::Enter => 0x0002,
        KeyCode::Left => 0x0003,
        KeyCode::Right => 0x0004,
        KeyCode::Up => 0x0005,
        KeyCode::Down => 0x0006,
        KeyCode::Home => 0x0007,
        KeyCode::End => 0x0008,
        KeyCode::PageUp => 0x0009,
        KeyCode::PageDown => 0x000A,
        KeyCode::Tab => 0x000B,
        KeyCode::BackTab => 0x000C,
        KeyCode::Delete => 0x000D,
        KeyCode::Insert => 0x000E,
        KeyCode::Escape => 0x000F,
        KeyCode::CapsLock => 0x0010,
        KeyCode::ScrollLock => 0x0011,
        KeyCode::NumLock => 0x0012,
        KeyCode::PrintScreen => 0x0013,
        KeyCode::Pause => 0x0014,
        KeyCode::Menu => 0x0015,
        KeyCode::KeypadBegin => 0x0016,
        // Media keys
        KeyCode::MediaPlay => 0x0020,
        KeyCode::MediaPause => 0x0021,
        KeyCode::MediaPlayPause => 0x0022,
        KeyCode::MediaStop => 0x0023,
        KeyCode::MediaReverse => 0x0024,
        KeyCode::MediaFastForward => 0x0025,
        KeyCode::MediaRewind => 0x0026,
        KeyCode::MediaNext => 0x0027,
        KeyCode::MediaPrevious => 0x0028,
        KeyCode::MediaRecord => 0x0029,
        KeyCode::MediaLowerVolume => 0x002A,
        KeyCode::MediaRaiseVolume => 0x002B,
        KeyCode::MediaMuteVolume => 0x002C,
        // Modifier keys (when reported as separate events)
        KeyCode::LeftShift => 0x0030,
        KeyCode::RightShift => 0x0031,
        KeyCode::LeftCtrl => 0x0032,
        KeyCode::RightCtrl => 0x0033,
        KeyCode::LeftAlt => 0x0034,
        KeyCode::RightAlt => 0x0035,
        KeyCode::LeftSuper => 0x0036,
        KeyCode::RightSuper => 0x0037,
        KeyCode::LeftHyper => 0x0038,
        KeyCode::RightHyper => 0x0039,
        KeyCode::LeftMeta => 0x003A,
        KeyCode::RightMeta => 0x003B,
        KeyCode::IsoLevel3Shift => 0x003C,
        KeyCode::IsoLevel5Shift => 0x003D,
        // Char and function keys (variable encoding)
        KeyCode::Char(c) => 0x1_0000 | u32::from(*c),
        KeyCode::F(n) => 0x200_0000 | u32::from(*n),
    }
}

fn u32_to_keycode(value: u32) -> KeyCode {
    match value {
        0x0000 => KeyCode::Null,
        0x0001 => KeyCode::Backspace,
        0x0002 => KeyCode::Enter,
        0x0003 => KeyCode::Left,
        0x0004 => KeyCode::Right,
        0x0005 => KeyCode::Up,
        0x0006 => KeyCode::Down,
        0x0007 => KeyCode::Home,
        0x0008 => KeyCode::End,
        0x0009 => KeyCode::PageUp,
        0x000A => KeyCode::PageDown,
        0x000B => KeyCode::Tab,
        0x000C => KeyCode::BackTab,
        0x000D => KeyCode::Delete,
        0x000E => KeyCode::Insert,
        0x000F => KeyCode::Escape,
        0x0010 => KeyCode::CapsLock,
        0x0011 => KeyCode::ScrollLock,
        0x0012 => KeyCode::NumLock,
        0x0013 => KeyCode::PrintScreen,
        0x0014 => KeyCode::Pause,
        0x0015 => KeyCode::Menu,
        0x0016 => KeyCode::KeypadBegin,
        // Media keys
        0x0020 => KeyCode::MediaPlay,
        0x0021 => KeyCode::MediaPause,
        0x0022 => KeyCode::MediaPlayPause,
        0x0023 => KeyCode::MediaStop,
        0x0024 => KeyCode::MediaReverse,
        0x0025 => KeyCode::MediaFastForward,
        0x0026 => KeyCode::MediaRewind,
        0x0027 => KeyCode::MediaNext,
        0x0028 => KeyCode::MediaPrevious,
        0x0029 => KeyCode::MediaRecord,
        0x002A => KeyCode::MediaLowerVolume,
        0x002B => KeyCode::MediaRaiseVolume,
        0x002C => KeyCode::MediaMuteVolume,
        // Modifier keys
        0x0030 => KeyCode::LeftShift,
        0x0031 => KeyCode::RightShift,
        0x0032 => KeyCode::LeftCtrl,
        0x0033 => KeyCode::RightCtrl,
        0x0034 => KeyCode::LeftAlt,
        0x0035 => KeyCode::RightAlt,
        0x0036 => KeyCode::LeftSuper,
        0x0037 => KeyCode::RightSuper,
        0x0038 => KeyCode::LeftHyper,
        0x0039 => KeyCode::RightHyper,
        0x003A => KeyCode::LeftMeta,
        0x003B => KeyCode::RightMeta,
        0x003C => KeyCode::IsoLevel3Shift,
        0x003D => KeyCode::IsoLevel5Shift,
        v if v & 0x200_0000 != 0 => {
            #[allow(clippy::cast_possible_truncation)]
            KeyCode::F((v & 0xFF) as u8)
        }
        v if v & 0x1_0000 != 0 => {
            let c = char::from_u32(v & 0xFFFF).unwrap_or('\0');
            KeyCode::Char(c)
        }
        _ => KeyCode::Null,
    }
}

/// Convert an arch-level `KeyCode` to the subsys-input `KeyCode`.
pub fn arch_keycode_to_subsys(code: &ArchKeyCode) -> KeyCode {
    match code {
        ArchKeyCode::Backspace => KeyCode::Backspace,
        ArchKeyCode::Enter => KeyCode::Enter,
        ArchKeyCode::Left => KeyCode::Left,
        ArchKeyCode::Right => KeyCode::Right,
        ArchKeyCode::Up => KeyCode::Up,
        ArchKeyCode::Down => KeyCode::Down,
        ArchKeyCode::Home => KeyCode::Home,
        ArchKeyCode::End => KeyCode::End,
        ArchKeyCode::PageUp => KeyCode::PageUp,
        ArchKeyCode::PageDown => KeyCode::PageDown,
        ArchKeyCode::Tab => KeyCode::Tab,
        ArchKeyCode::BackTab => KeyCode::BackTab,
        ArchKeyCode::Delete => KeyCode::Delete,
        ArchKeyCode::Insert => KeyCode::Insert,
        ArchKeyCode::Escape => KeyCode::Escape,
        ArchKeyCode::Char(c) => KeyCode::Char(*c),
        ArchKeyCode::F(n) => KeyCode::F(*n),
        ArchKeyCode::Null => KeyCode::Null,
        ArchKeyCode::CapsLock => KeyCode::CapsLock,
        ArchKeyCode::ScrollLock => KeyCode::ScrollLock,
        ArchKeyCode::NumLock => KeyCode::NumLock,
        ArchKeyCode::PrintScreen => KeyCode::PrintScreen,
        ArchKeyCode::Pause => KeyCode::Pause,
        ArchKeyCode::Menu => KeyCode::Menu,
        ArchKeyCode::KeypadBegin => KeyCode::KeypadBegin,
        ArchKeyCode::MediaPlay => KeyCode::MediaPlay,
        ArchKeyCode::MediaPause => KeyCode::MediaPause,
        ArchKeyCode::MediaPlayPause => KeyCode::MediaPlayPause,
        ArchKeyCode::MediaStop => KeyCode::MediaStop,
        ArchKeyCode::MediaReverse => KeyCode::MediaReverse,
        ArchKeyCode::MediaFastForward => KeyCode::MediaFastForward,
        ArchKeyCode::MediaRewind => KeyCode::MediaRewind,
        ArchKeyCode::MediaNext => KeyCode::MediaNext,
        ArchKeyCode::MediaPrevious => KeyCode::MediaPrevious,
        ArchKeyCode::MediaRecord => KeyCode::MediaRecord,
        ArchKeyCode::MediaLowerVolume => KeyCode::MediaLowerVolume,
        ArchKeyCode::MediaRaiseVolume => KeyCode::MediaRaiseVolume,
        ArchKeyCode::MediaMuteVolume => KeyCode::MediaMuteVolume,
        ArchKeyCode::LeftShift => KeyCode::LeftShift,
        ArchKeyCode::RightShift => KeyCode::RightShift,
        ArchKeyCode::LeftCtrl => KeyCode::LeftCtrl,
        ArchKeyCode::RightCtrl => KeyCode::RightCtrl,
        ArchKeyCode::LeftAlt => KeyCode::LeftAlt,
        ArchKeyCode::RightAlt => KeyCode::RightAlt,
        ArchKeyCode::LeftSuper => KeyCode::LeftSuper,
        ArchKeyCode::RightSuper => KeyCode::RightSuper,
        ArchKeyCode::LeftHyper => KeyCode::LeftHyper,
        ArchKeyCode::RightHyper => KeyCode::RightHyper,
        ArchKeyCode::LeftMeta => KeyCode::LeftMeta,
        ArchKeyCode::RightMeta => KeyCode::RightMeta,
        ArchKeyCode::IsoLevel3Shift => KeyCode::IsoLevel3Shift,
        ArchKeyCode::IsoLevel5Shift => KeyCode::IsoLevel5Shift,
    }
}
