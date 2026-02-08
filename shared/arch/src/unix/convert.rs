//! Type conversions from crossterm types to our platform-agnostic types.

use {
    crate::traits::{
        ClearType, Color, InputEvent, KeyCode, KeyEvent, KeyEventKind, KeyEventState, Modifiers,
        MouseButton, MouseEvent, MouseEventKind, TerminalSize,
    },
    crossterm::{
        event::{self as ct_event, Event as CtEvent},
        style::Color as CtColor,
    },
};

/// Convert a crossterm event to our `InputEvent`.
#[must_use]
pub fn convert_event(event: CtEvent) -> InputEvent {
    match event {
        CtEvent::Key(k) => InputEvent::Key(convert_key_event(k)),
        CtEvent::Mouse(m) => InputEvent::Mouse(convert_mouse_event(m)),
        CtEvent::Resize(cols, rows) => InputEvent::Resize(TerminalSize::new(cols, rows)),
        CtEvent::FocusGained => InputEvent::FocusGained,
        CtEvent::FocusLost => InputEvent::FocusLost,
        CtEvent::Paste(s) => InputEvent::Paste(s),
    }
}

/// Convert a crossterm `KeyEvent` to our `KeyEvent`.
#[must_use]
pub fn convert_key_event(k: ct_event::KeyEvent) -> KeyEvent {
    KeyEvent {
        code: convert_key_code(k.code),
        modifiers: convert_modifiers(k.modifiers),
        kind: convert_key_kind(k.kind),
        state: convert_key_state(k.state),
    }
}

/// Convert crossterm `KeyCode` to our `KeyCode`.
#[must_use]
pub const fn convert_key_code(code: ct_event::KeyCode) -> KeyCode {
    use ct_event::KeyCode as Ct;
    match code {
        Ct::Char(c) => KeyCode::Char(c),
        Ct::F(n) => KeyCode::F(n),
        Ct::Backspace => KeyCode::Backspace,
        Ct::Enter => KeyCode::Enter,
        Ct::Tab => KeyCode::Tab,
        Ct::BackTab => KeyCode::BackTab,
        Ct::Esc => KeyCode::Escape,
        Ct::Up => KeyCode::Up,
        Ct::Down => KeyCode::Down,
        Ct::Left => KeyCode::Left,
        Ct::Right => KeyCode::Right,
        Ct::Home => KeyCode::Home,
        Ct::End => KeyCode::End,
        Ct::PageUp => KeyCode::PageUp,
        Ct::PageDown => KeyCode::PageDown,
        Ct::Insert => KeyCode::Insert,
        Ct::Delete => KeyCode::Delete,
        Ct::Null => KeyCode::Null,
        Ct::CapsLock => KeyCode::CapsLock,
        Ct::ScrollLock => KeyCode::ScrollLock,
        Ct::NumLock => KeyCode::NumLock,
        Ct::PrintScreen => KeyCode::PrintScreen,
        Ct::Pause => KeyCode::Pause,
        Ct::Menu => KeyCode::Menu,
        Ct::KeypadBegin => KeyCode::KeypadBegin,
        Ct::Media(media) => convert_media_key(media),
        Ct::Modifier(modifier) => convert_modifier_key(modifier),
    }
}

/// Convert crossterm `MediaKeyCode` to our `KeyCode`.
const fn convert_media_key(media: ct_event::MediaKeyCode) -> KeyCode {
    use ct_event::MediaKeyCode as Ct;
    match media {
        Ct::Play => KeyCode::MediaPlay,
        Ct::Pause => KeyCode::MediaPause,
        Ct::PlayPause => KeyCode::MediaPlayPause,
        Ct::Stop => KeyCode::MediaStop,
        Ct::Reverse => KeyCode::MediaReverse,
        Ct::FastForward => KeyCode::MediaFastForward,
        Ct::Rewind => KeyCode::MediaRewind,
        Ct::TrackNext => KeyCode::MediaNext,
        Ct::TrackPrevious => KeyCode::MediaPrevious,
        Ct::Record => KeyCode::MediaRecord,
        Ct::LowerVolume => KeyCode::MediaLowerVolume,
        Ct::RaiseVolume => KeyCode::MediaRaiseVolume,
        Ct::MuteVolume => KeyCode::MediaMuteVolume,
    }
}

/// Convert crossterm `ModifierKeyCode` to our `KeyCode`.
const fn convert_modifier_key(modifier: ct_event::ModifierKeyCode) -> KeyCode {
    use ct_event::ModifierKeyCode as Ct;
    match modifier {
        Ct::LeftShift => KeyCode::LeftShift,
        Ct::RightShift => KeyCode::RightShift,
        Ct::LeftControl => KeyCode::LeftCtrl,
        Ct::RightControl => KeyCode::RightCtrl,
        Ct::LeftAlt => KeyCode::LeftAlt,
        Ct::RightAlt => KeyCode::RightAlt,
        Ct::LeftSuper => KeyCode::LeftSuper,
        Ct::RightSuper => KeyCode::RightSuper,
        Ct::LeftHyper => KeyCode::LeftHyper,
        Ct::RightHyper => KeyCode::RightHyper,
        Ct::LeftMeta => KeyCode::LeftMeta,
        Ct::RightMeta => KeyCode::RightMeta,
        Ct::IsoLevel3Shift => KeyCode::IsoLevel3Shift,
        Ct::IsoLevel5Shift => KeyCode::IsoLevel5Shift,
    }
}

/// Convert crossterm `KeyModifiers` to our `Modifiers`.
#[must_use]
pub fn convert_modifiers(mods: ct_event::KeyModifiers) -> Modifiers {
    let mut result = Modifiers::NONE;
    if mods.contains(ct_event::KeyModifiers::SHIFT) {
        result |= Modifiers::SHIFT;
    }
    if mods.contains(ct_event::KeyModifiers::CONTROL) {
        result |= Modifiers::CTRL;
    }
    if mods.contains(ct_event::KeyModifiers::ALT) {
        result |= Modifiers::ALT;
    }
    if mods.contains(ct_event::KeyModifiers::SUPER) {
        result |= Modifiers::SUPER;
    }
    if mods.contains(ct_event::KeyModifiers::HYPER) {
        result |= Modifiers::HYPER;
    }
    if mods.contains(ct_event::KeyModifiers::META) {
        result |= Modifiers::META;
    }
    result
}

/// Convert crossterm `KeyEventKind` to our `KeyEventKind`.
pub const fn convert_key_kind(kind: ct_event::KeyEventKind) -> KeyEventKind {
    match kind {
        ct_event::KeyEventKind::Press => KeyEventKind::Press,
        ct_event::KeyEventKind::Repeat => KeyEventKind::Repeat,
        ct_event::KeyEventKind::Release => KeyEventKind::Release,
    }
}

/// Convert crossterm `KeyEventState` to our `KeyEventState`.
#[allow(clippy::missing_const_for_fn)] // bitflags contains() isn't const
pub fn convert_key_state(state: ct_event::KeyEventState) -> KeyEventState {
    let mut result = KeyEventState::NONE;
    if state.contains(ct_event::KeyEventState::KEYPAD) {
        result = result.union(KeyEventState::KEYPAD);
    }
    if state.contains(ct_event::KeyEventState::CAPS_LOCK) {
        result = result.union(KeyEventState::CAPS_LOCK);
    }
    if state.contains(ct_event::KeyEventState::NUM_LOCK) {
        result = result.union(KeyEventState::NUM_LOCK);
    }
    result
}

/// Convert a crossterm `MouseEvent` to our `MouseEvent`.
pub fn convert_mouse_event(m: ct_event::MouseEvent) -> MouseEvent {
    MouseEvent {
        kind: convert_mouse_kind(m.kind),
        column: m.column,
        row: m.row,
        modifiers: convert_modifiers(m.modifiers),
    }
}

/// Convert crossterm `MouseEventKind` to our `MouseEventKind`.
const fn convert_mouse_kind(kind: ct_event::MouseEventKind) -> MouseEventKind {
    use ct_event::MouseEventKind as Ct;
    match kind {
        Ct::Down(btn) => MouseEventKind::Down(convert_mouse_button(btn)),
        Ct::Up(btn) => MouseEventKind::Up(convert_mouse_button(btn)),
        Ct::Drag(btn) => MouseEventKind::Drag(convert_mouse_button(btn)),
        Ct::Moved => MouseEventKind::Moved,
        Ct::ScrollUp => MouseEventKind::ScrollUp,
        Ct::ScrollDown => MouseEventKind::ScrollDown,
        Ct::ScrollLeft => MouseEventKind::ScrollLeft,
        Ct::ScrollRight => MouseEventKind::ScrollRight,
    }
}

/// Convert crossterm `MouseButton` to our `MouseButton`.
const fn convert_mouse_button(btn: ct_event::MouseButton) -> MouseButton {
    match btn {
        ct_event::MouseButton::Left => MouseButton::Left,
        ct_event::MouseButton::Right => MouseButton::Right,
        ct_event::MouseButton::Middle => MouseButton::Middle,
    }
}

/// Convert our `ClearType` to crossterm `ClearType`.
pub const fn convert_clear_type(ct: ClearType) -> crossterm::terminal::ClearType {
    use crossterm::terminal::ClearType as CtClear;
    match ct {
        ClearType::All => CtClear::All,
        ClearType::FromCursorDown => CtClear::FromCursorDown,
        ClearType::FromCursorUp => CtClear::FromCursorUp,
        ClearType::CurrentLine => CtClear::CurrentLine,
        ClearType::UntilNewLine => CtClear::UntilNewLine,
        ClearType::Purge => CtClear::Purge,
    }
}

/// Convert crossterm `Color` to our `Color`.
#[must_use]
pub const fn convert_color_from_crossterm(color: CtColor) -> Color {
    match color {
        CtColor::Reset => Color::Reset,
        CtColor::Black => Color::Black,
        CtColor::DarkRed => Color::DarkRed,
        CtColor::DarkGreen => Color::DarkGreen,
        CtColor::DarkYellow => Color::DarkYellow,
        CtColor::DarkBlue => Color::DarkBlue,
        CtColor::DarkMagenta => Color::DarkMagenta,
        CtColor::DarkCyan => Color::DarkCyan,
        CtColor::Grey => Color::Grey,
        CtColor::DarkGrey => Color::DarkGrey,
        CtColor::Red => Color::Red,
        CtColor::Green => Color::Green,
        CtColor::Yellow => Color::Yellow,
        CtColor::Blue => Color::Blue,
        CtColor::Magenta => Color::Magenta,
        CtColor::Cyan => Color::Cyan,
        CtColor::White => Color::White,
        CtColor::AnsiValue(n) => Color::AnsiValue(n),
        CtColor::Rgb { r, g, b } => Color::Rgb { r, g, b },
    }
}

/// Convert our `Color` to crossterm `Color`.
#[must_use]
pub const fn convert_color_to_crossterm(color: Color) -> CtColor {
    match color {
        Color::Reset => CtColor::Reset,
        Color::Black => CtColor::Black,
        Color::DarkRed => CtColor::DarkRed,
        Color::DarkGreen => CtColor::DarkGreen,
        Color::DarkYellow => CtColor::DarkYellow,
        Color::DarkBlue => CtColor::DarkBlue,
        Color::DarkMagenta => CtColor::DarkMagenta,
        Color::DarkCyan => CtColor::DarkCyan,
        Color::Grey => CtColor::Grey,
        Color::DarkGrey => CtColor::DarkGrey,
        Color::Red => CtColor::Red,
        Color::Green => CtColor::Green,
        Color::Yellow => CtColor::Yellow,
        Color::Blue => CtColor::Blue,
        Color::Magenta => CtColor::Magenta,
        Color::Cyan => CtColor::Cyan,
        Color::White => CtColor::White,
        Color::AnsiValue(n) => CtColor::AnsiValue(n),
        Color::Rgb { r, g, b } => CtColor::Rgb { r, g, b },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_key_code_char() {
        let ct_code = ct_event::KeyCode::Char('a');
        let our_code = convert_key_code(ct_code);
        assert_eq!(our_code, KeyCode::Char('a'));
    }

    #[test]
    fn test_convert_modifiers() {
        let ct_mods = ct_event::KeyModifiers::CONTROL | ct_event::KeyModifiers::SHIFT;
        let our_mods = convert_modifiers(ct_mods);
        assert!(our_mods.contains(Modifiers::CTRL));
        assert!(our_mods.contains(Modifiers::SHIFT));
        assert!(!our_mods.contains(Modifiers::ALT));
    }

    #[test]
    fn test_convert_key_event() {
        let ct_event =
            ct_event::KeyEvent::new(ct_event::KeyCode::Esc, ct_event::KeyModifiers::NONE);
        let our_event = convert_key_event(ct_event);
        assert_eq!(our_event.code, KeyCode::Escape);
        assert_eq!(our_event.modifiers, Modifiers::NONE);
        assert_eq!(our_event.kind, KeyEventKind::Press);
    }

    #[test]
    fn test_convert_mouse_event() {
        let ct_mouse = ct_event::MouseEvent {
            kind: ct_event::MouseEventKind::Down(ct_event::MouseButton::Left),
            column: 10,
            row: 5,
            modifiers: ct_event::KeyModifiers::NONE,
        };
        let our_mouse = convert_mouse_event(ct_mouse);
        assert!(matches!(our_mouse.kind, MouseEventKind::Down(MouseButton::Left)));
        assert_eq!(our_mouse.column, 10);
        assert_eq!(our_mouse.row, 5);
    }

    #[test]
    fn test_convert_clear_type() {
        assert!(matches!(
            convert_clear_type(ClearType::All),
            crossterm::terminal::ClearType::All
        ));
    }

    // =========================================================================
    // Comprehensive KeyCode conversions
    // =========================================================================

    #[test]
    fn test_convert_key_code_special_keys() {
        assert_eq!(convert_key_code(ct_event::KeyCode::Backspace), KeyCode::Backspace);
        assert_eq!(convert_key_code(ct_event::KeyCode::Enter), KeyCode::Enter);
        assert_eq!(convert_key_code(ct_event::KeyCode::Tab), KeyCode::Tab);
        assert_eq!(convert_key_code(ct_event::KeyCode::BackTab), KeyCode::BackTab);
        assert_eq!(convert_key_code(ct_event::KeyCode::Esc), KeyCode::Escape);
        assert_eq!(convert_key_code(ct_event::KeyCode::Null), KeyCode::Null);
    }

    #[test]
    fn test_convert_key_code_arrow_keys() {
        assert_eq!(convert_key_code(ct_event::KeyCode::Up), KeyCode::Up);
        assert_eq!(convert_key_code(ct_event::KeyCode::Down), KeyCode::Down);
        assert_eq!(convert_key_code(ct_event::KeyCode::Left), KeyCode::Left);
        assert_eq!(convert_key_code(ct_event::KeyCode::Right), KeyCode::Right);
    }

    #[test]
    fn test_convert_key_code_navigation() {
        assert_eq!(convert_key_code(ct_event::KeyCode::Home), KeyCode::Home);
        assert_eq!(convert_key_code(ct_event::KeyCode::End), KeyCode::End);
        assert_eq!(convert_key_code(ct_event::KeyCode::PageUp), KeyCode::PageUp);
        assert_eq!(convert_key_code(ct_event::KeyCode::PageDown), KeyCode::PageDown);
        assert_eq!(convert_key_code(ct_event::KeyCode::Insert), KeyCode::Insert);
        assert_eq!(convert_key_code(ct_event::KeyCode::Delete), KeyCode::Delete);
    }

    #[test]
    fn test_convert_key_code_lock_keys() {
        assert_eq!(convert_key_code(ct_event::KeyCode::CapsLock), KeyCode::CapsLock);
        assert_eq!(convert_key_code(ct_event::KeyCode::ScrollLock), KeyCode::ScrollLock);
        assert_eq!(convert_key_code(ct_event::KeyCode::NumLock), KeyCode::NumLock);
    }

    #[test]
    fn test_convert_key_code_misc() {
        assert_eq!(convert_key_code(ct_event::KeyCode::PrintScreen), KeyCode::PrintScreen);
        assert_eq!(convert_key_code(ct_event::KeyCode::Pause), KeyCode::Pause);
        assert_eq!(convert_key_code(ct_event::KeyCode::Menu), KeyCode::Menu);
        assert_eq!(convert_key_code(ct_event::KeyCode::KeypadBegin), KeyCode::KeypadBegin);
    }

    #[test]
    fn test_convert_key_code_function_keys() {
        for n in 1..=12 {
            assert_eq!(convert_key_code(ct_event::KeyCode::F(n)), KeyCode::F(n));
        }
    }

    // =========================================================================
    // Media key conversions
    // =========================================================================

    #[test]
    fn test_convert_media_keys() {
        use ct_event::MediaKeyCode as Ct;
        let mappings = [
            (Ct::Play, KeyCode::MediaPlay),
            (Ct::Pause, KeyCode::MediaPause),
            (Ct::PlayPause, KeyCode::MediaPlayPause),
            (Ct::Stop, KeyCode::MediaStop),
            (Ct::Reverse, KeyCode::MediaReverse),
            (Ct::FastForward, KeyCode::MediaFastForward),
            (Ct::Rewind, KeyCode::MediaRewind),
            (Ct::TrackNext, KeyCode::MediaNext),
            (Ct::TrackPrevious, KeyCode::MediaPrevious),
            (Ct::Record, KeyCode::MediaRecord),
            (Ct::LowerVolume, KeyCode::MediaLowerVolume),
            (Ct::RaiseVolume, KeyCode::MediaRaiseVolume),
            (Ct::MuteVolume, KeyCode::MediaMuteVolume),
        ];
        for (ct, expected) in mappings {
            let ct_code = ct_event::KeyCode::Media(ct);
            assert_eq!(convert_key_code(ct_code), expected, "Media key mismatch for {ct:?}");
        }
    }

    // =========================================================================
    // Modifier key conversions
    // =========================================================================

    #[test]
    fn test_convert_modifier_keys() {
        use ct_event::ModifierKeyCode as Ct;
        let mappings = [
            (Ct::LeftShift, KeyCode::LeftShift),
            (Ct::RightShift, KeyCode::RightShift),
            (Ct::LeftControl, KeyCode::LeftCtrl),
            (Ct::RightControl, KeyCode::RightCtrl),
            (Ct::LeftAlt, KeyCode::LeftAlt),
            (Ct::RightAlt, KeyCode::RightAlt),
            (Ct::LeftSuper, KeyCode::LeftSuper),
            (Ct::RightSuper, KeyCode::RightSuper),
            (Ct::LeftHyper, KeyCode::LeftHyper),
            (Ct::RightHyper, KeyCode::RightHyper),
            (Ct::LeftMeta, KeyCode::LeftMeta),
            (Ct::RightMeta, KeyCode::RightMeta),
            (Ct::IsoLevel3Shift, KeyCode::IsoLevel3Shift),
            (Ct::IsoLevel5Shift, KeyCode::IsoLevel5Shift),
        ];
        for (ct, expected) in mappings {
            let ct_code = ct_event::KeyCode::Modifier(ct);
            assert_eq!(convert_key_code(ct_code), expected, "Modifier key mismatch for {ct:?}");
        }
    }

    // =========================================================================
    // Modifier flags conversions
    // =========================================================================

    #[test]
    fn test_convert_modifiers_individual() {
        let shift = convert_modifiers(ct_event::KeyModifiers::SHIFT);
        assert!(shift.contains(Modifiers::SHIFT));
        assert!(!shift.contains(Modifiers::CTRL));

        let ctrl = convert_modifiers(ct_event::KeyModifiers::CONTROL);
        assert!(ctrl.contains(Modifiers::CTRL));

        let alt = convert_modifiers(ct_event::KeyModifiers::ALT);
        assert!(alt.contains(Modifiers::ALT));

        let super_mod = convert_modifiers(ct_event::KeyModifiers::SUPER);
        assert!(super_mod.contains(Modifiers::SUPER));

        let hyper = convert_modifiers(ct_event::KeyModifiers::HYPER);
        assert!(hyper.contains(Modifiers::HYPER));

        let meta = convert_modifiers(ct_event::KeyModifiers::META);
        assert!(meta.contains(Modifiers::META));
    }

    #[test]
    fn test_convert_modifiers_none() {
        let mods = convert_modifiers(ct_event::KeyModifiers::NONE);
        assert!(mods.is_empty());
    }

    #[test]
    fn test_convert_modifiers_all_combined() {
        let ct_all = ct_event::KeyModifiers::SHIFT
            | ct_event::KeyModifiers::CONTROL
            | ct_event::KeyModifiers::ALT
            | ct_event::KeyModifiers::SUPER
            | ct_event::KeyModifiers::HYPER
            | ct_event::KeyModifiers::META;
        let mods = convert_modifiers(ct_all);
        assert!(mods.contains(Modifiers::SHIFT));
        assert!(mods.contains(Modifiers::CTRL));
        assert!(mods.contains(Modifiers::ALT));
        assert!(mods.contains(Modifiers::SUPER));
        assert!(mods.contains(Modifiers::HYPER));
        assert!(mods.contains(Modifiers::META));
    }

    // =========================================================================
    // Key event kind conversions
    // =========================================================================

    #[test]
    fn test_convert_key_kind_all() {
        assert_eq!(convert_key_kind(ct_event::KeyEventKind::Press), KeyEventKind::Press);
        assert_eq!(convert_key_kind(ct_event::KeyEventKind::Repeat), KeyEventKind::Repeat);
        assert_eq!(convert_key_kind(ct_event::KeyEventKind::Release), KeyEventKind::Release);
    }

    // =========================================================================
    // Key event state conversions
    // =========================================================================

    #[test]
    fn test_convert_key_state_none() {
        let state = convert_key_state(ct_event::KeyEventState::NONE);
        assert!(state.is_empty());
    }

    #[test]
    fn test_convert_key_state_keypad() {
        let state = convert_key_state(ct_event::KeyEventState::KEYPAD);
        assert!(state.contains(KeyEventState::KEYPAD));
        assert!(!state.contains(KeyEventState::CAPS_LOCK));
        assert!(!state.contains(KeyEventState::NUM_LOCK));
    }

    #[test]
    fn test_convert_key_state_caps_lock() {
        let state = convert_key_state(ct_event::KeyEventState::CAPS_LOCK);
        assert!(state.contains(KeyEventState::CAPS_LOCK));
    }

    #[test]
    fn test_convert_key_state_num_lock() {
        let state = convert_key_state(ct_event::KeyEventState::NUM_LOCK);
        assert!(state.contains(KeyEventState::NUM_LOCK));
    }

    #[test]
    fn test_convert_key_state_combined() {
        let ct_state = ct_event::KeyEventState::KEYPAD
            | ct_event::KeyEventState::CAPS_LOCK
            | ct_event::KeyEventState::NUM_LOCK;
        let state = convert_key_state(ct_state);
        assert!(state.contains(KeyEventState::KEYPAD));
        assert!(state.contains(KeyEventState::CAPS_LOCK));
        assert!(state.contains(KeyEventState::NUM_LOCK));
    }

    // =========================================================================
    // Mouse event conversions
    // =========================================================================

    #[test]
    fn test_convert_mouse_event_all_buttons() {
        for (ct_btn, our_btn) in [
            (ct_event::MouseButton::Left, MouseButton::Left),
            (ct_event::MouseButton::Right, MouseButton::Right),
            (ct_event::MouseButton::Middle, MouseButton::Middle),
        ] {
            let ct_mouse = ct_event::MouseEvent {
                kind: ct_event::MouseEventKind::Down(ct_btn),
                column: 1,
                row: 2,
                modifiers: ct_event::KeyModifiers::NONE,
            };
            let our_mouse = convert_mouse_event(ct_mouse);
            assert!(matches!(our_mouse.kind, MouseEventKind::Down(b) if b == our_btn));
        }
    }

    #[test]
    fn test_convert_mouse_event_up_drag() {
        let ct_up = ct_event::MouseEvent {
            kind: ct_event::MouseEventKind::Up(ct_event::MouseButton::Right),
            column: 5,
            row: 10,
            modifiers: ct_event::KeyModifiers::NONE,
        };
        let our_up = convert_mouse_event(ct_up);
        assert!(matches!(our_up.kind, MouseEventKind::Up(MouseButton::Right)));

        let ct_drag = ct_event::MouseEvent {
            kind: ct_event::MouseEventKind::Drag(ct_event::MouseButton::Middle),
            column: 3,
            row: 7,
            modifiers: ct_event::KeyModifiers::SHIFT,
        };
        let our_drag = convert_mouse_event(ct_drag);
        assert!(matches!(our_drag.kind, MouseEventKind::Drag(MouseButton::Middle)));
        assert!(our_drag.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn test_convert_mouse_event_scroll() {
        for (ct_kind, expected_kind) in [
            (ct_event::MouseEventKind::ScrollUp, MouseEventKind::ScrollUp),
            (ct_event::MouseEventKind::ScrollDown, MouseEventKind::ScrollDown),
            (ct_event::MouseEventKind::ScrollLeft, MouseEventKind::ScrollLeft),
            (ct_event::MouseEventKind::ScrollRight, MouseEventKind::ScrollRight),
        ] {
            let ct_mouse = ct_event::MouseEvent {
                kind: ct_kind,
                column: 0,
                row: 0,
                modifiers: ct_event::KeyModifiers::NONE,
            };
            let our_mouse = convert_mouse_event(ct_mouse);
            assert_eq!(our_mouse.kind, expected_kind, "Scroll kind mismatch for {ct_kind:?}");
        }
    }

    #[test]
    fn test_convert_mouse_event_moved() {
        let ct_moved = ct_event::MouseEvent {
            kind: ct_event::MouseEventKind::Moved,
            column: 42,
            row: 13,
            modifiers: ct_event::KeyModifiers::NONE,
        };
        let our_moved = convert_mouse_event(ct_moved);
        assert_eq!(our_moved.kind, MouseEventKind::Moved);
        assert_eq!(our_moved.column, 42);
        assert_eq!(our_moved.row, 13);
    }

    // =========================================================================
    // Full event conversions
    // =========================================================================

    #[test]
    fn test_convert_event_key() {
        let ct = CtEvent::Key(ct_event::KeyEvent::new(
            ct_event::KeyCode::Char('z'),
            ct_event::KeyModifiers::ALT,
        ));
        let our = convert_event(ct);
        match our {
            InputEvent::Key(key) => {
                assert_eq!(key.code, KeyCode::Char('z'));
                assert!(key.modifiers.contains(Modifiers::ALT));
            }
            _ => panic!("Expected Key event"),
        }
    }

    #[test]
    fn test_convert_event_mouse() {
        let ct = CtEvent::Mouse(ct_event::MouseEvent {
            kind: ct_event::MouseEventKind::Down(ct_event::MouseButton::Left),
            column: 10,
            row: 20,
            modifiers: ct_event::KeyModifiers::NONE,
        });
        let our = convert_event(ct);
        assert!(matches!(our, InputEvent::Mouse(_)));
    }

    #[test]
    fn test_convert_event_resize() {
        let ct = CtEvent::Resize(120, 40);
        let our = convert_event(ct);
        match our {
            InputEvent::Resize(size) => {
                assert_eq!(size.cols, 120);
                assert_eq!(size.rows, 40);
            }
            _ => panic!("Expected Resize event"),
        }
    }

    #[test]
    fn test_convert_event_focus() {
        let gained = convert_event(CtEvent::FocusGained);
        assert!(matches!(gained, InputEvent::FocusGained));

        let lost = convert_event(CtEvent::FocusLost);
        assert!(matches!(lost, InputEvent::FocusLost));
    }

    #[test]
    fn test_convert_event_paste() {
        let ct = CtEvent::Paste("pasted text".to_string());
        let our = convert_event(ct);
        match our {
            InputEvent::Paste(text) => assert_eq!(text, "pasted text"),
            _ => panic!("Expected Paste event"),
        }
    }

    // =========================================================================
    // Clear type conversions - all variants
    // =========================================================================

    #[test]
    fn test_convert_clear_type_all_variants() {
        use crossterm::terminal::ClearType as CtClear;
        assert!(matches!(convert_clear_type(ClearType::All), CtClear::All));
        assert!(matches!(convert_clear_type(ClearType::FromCursorDown), CtClear::FromCursorDown));
        assert!(matches!(convert_clear_type(ClearType::FromCursorUp), CtClear::FromCursorUp));
        assert!(matches!(convert_clear_type(ClearType::CurrentLine), CtClear::CurrentLine));
        assert!(matches!(convert_clear_type(ClearType::UntilNewLine), CtClear::UntilNewLine));
        assert!(matches!(convert_clear_type(ClearType::Purge), CtClear::Purge));
    }

    // =========================================================================
    // Color conversions - crossterm <-> reovim
    // =========================================================================

    #[test]
    fn test_convert_color_from_crossterm_named() {
        assert_eq!(convert_color_from_crossterm(CtColor::Reset), Color::Reset);
        assert_eq!(convert_color_from_crossterm(CtColor::Black), Color::Black);
        assert_eq!(convert_color_from_crossterm(CtColor::DarkRed), Color::DarkRed);
        assert_eq!(convert_color_from_crossterm(CtColor::DarkGreen), Color::DarkGreen);
        assert_eq!(convert_color_from_crossterm(CtColor::DarkYellow), Color::DarkYellow);
        assert_eq!(convert_color_from_crossterm(CtColor::DarkBlue), Color::DarkBlue);
        assert_eq!(convert_color_from_crossterm(CtColor::DarkMagenta), Color::DarkMagenta);
        assert_eq!(convert_color_from_crossterm(CtColor::DarkCyan), Color::DarkCyan);
        assert_eq!(convert_color_from_crossterm(CtColor::Grey), Color::Grey);
        assert_eq!(convert_color_from_crossterm(CtColor::DarkGrey), Color::DarkGrey);
        assert_eq!(convert_color_from_crossterm(CtColor::Red), Color::Red);
        assert_eq!(convert_color_from_crossterm(CtColor::Green), Color::Green);
        assert_eq!(convert_color_from_crossterm(CtColor::Yellow), Color::Yellow);
        assert_eq!(convert_color_from_crossterm(CtColor::Blue), Color::Blue);
        assert_eq!(convert_color_from_crossterm(CtColor::Magenta), Color::Magenta);
        assert_eq!(convert_color_from_crossterm(CtColor::Cyan), Color::Cyan);
        assert_eq!(convert_color_from_crossterm(CtColor::White), Color::White);
    }

    #[test]
    fn test_convert_color_from_crossterm_ansi() {
        assert_eq!(convert_color_from_crossterm(CtColor::AnsiValue(0)), Color::AnsiValue(0));
        assert_eq!(convert_color_from_crossterm(CtColor::AnsiValue(128)), Color::AnsiValue(128));
        assert_eq!(convert_color_from_crossterm(CtColor::AnsiValue(255)), Color::AnsiValue(255));
    }

    #[test]
    fn test_convert_color_from_crossterm_rgb() {
        assert_eq!(
            convert_color_from_crossterm(CtColor::Rgb {
                r: 100,
                g: 200,
                b: 50
            }),
            Color::Rgb {
                r: 100,
                g: 200,
                b: 50
            }
        );
    }

    #[test]
    fn test_convert_color_to_crossterm_named() {
        assert_eq!(convert_color_to_crossterm(Color::Reset), CtColor::Reset);
        assert_eq!(convert_color_to_crossterm(Color::Black), CtColor::Black);
        assert_eq!(convert_color_to_crossterm(Color::DarkRed), CtColor::DarkRed);
        assert_eq!(convert_color_to_crossterm(Color::DarkGreen), CtColor::DarkGreen);
        assert_eq!(convert_color_to_crossterm(Color::DarkYellow), CtColor::DarkYellow);
        assert_eq!(convert_color_to_crossterm(Color::DarkBlue), CtColor::DarkBlue);
        assert_eq!(convert_color_to_crossterm(Color::DarkMagenta), CtColor::DarkMagenta);
        assert_eq!(convert_color_to_crossterm(Color::DarkCyan), CtColor::DarkCyan);
        assert_eq!(convert_color_to_crossterm(Color::Grey), CtColor::Grey);
        assert_eq!(convert_color_to_crossterm(Color::DarkGrey), CtColor::DarkGrey);
        assert_eq!(convert_color_to_crossterm(Color::Red), CtColor::Red);
        assert_eq!(convert_color_to_crossterm(Color::Green), CtColor::Green);
        assert_eq!(convert_color_to_crossterm(Color::Yellow), CtColor::Yellow);
        assert_eq!(convert_color_to_crossterm(Color::Blue), CtColor::Blue);
        assert_eq!(convert_color_to_crossterm(Color::Magenta), CtColor::Magenta);
        assert_eq!(convert_color_to_crossterm(Color::Cyan), CtColor::Cyan);
        assert_eq!(convert_color_to_crossterm(Color::White), CtColor::White);
    }

    #[test]
    fn test_convert_color_to_crossterm_ansi() {
        assert_eq!(convert_color_to_crossterm(Color::AnsiValue(42)), CtColor::AnsiValue(42));
    }

    #[test]
    fn test_convert_color_to_crossterm_rgb() {
        assert_eq!(
            convert_color_to_crossterm(Color::Rgb {
                r: 10,
                g: 20,
                b: 30
            }),
            CtColor::Rgb {
                r: 10,
                g: 20,
                b: 30
            }
        );
    }

    #[test]
    fn test_color_roundtrip_crossterm() {
        // All named colors should roundtrip through crossterm conversion
        let colors = [
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
            Color::AnsiValue(0),
            Color::AnsiValue(128),
            Color::AnsiValue(255),
            Color::Rgb { r: 0, g: 0, b: 0 },
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
            Color::Rgb {
                r: 100,
                g: 150,
                b: 200,
            },
        ];
        for color in colors {
            let ct = convert_color_to_crossterm(color);
            let back = convert_color_from_crossterm(ct);
            assert_eq!(color, back, "Color roundtrip failed for {color:?} -> {ct:?} -> {back:?}");
        }
    }
}
