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
#[path = "convert_tests.rs"]
mod tests;
