//! Temporary local↔legacy adapters for Mission #753 Plan 10 Phase 1.
//!
//! `reovim-input-codec` now owns the typed key/mouse vocabulary, but legacy
//! consumers still use the retained overlap in `reovim-subsys-input`. These
//! adapters keep the boundary explicit until later phases repoint or delete the
//! remaining legacy consumers.

use crate::{
    KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

impl From<reovim_subsys_input::Modifiers> for Modifiers {
    fn from(modifiers: reovim_subsys_input::Modifiers) -> Self {
        Self::from_bits_retain(modifiers.bits())
    }
}

impl From<Modifiers> for reovim_subsys_input::Modifiers {
    fn from(modifiers: Modifiers) -> Self {
        Self::from_bits_retain(modifiers.bits())
    }
}

impl From<reovim_subsys_input::KeyCode> for KeyCode {
    fn from(code: reovim_subsys_input::KeyCode) -> Self {
        match code {
            reovim_subsys_input::KeyCode::Char(c) => Self::Char(c),
            reovim_subsys_input::KeyCode::F(n) => Self::F(n),
            reovim_subsys_input::KeyCode::Up => Self::Up,
            reovim_subsys_input::KeyCode::Down => Self::Down,
            reovim_subsys_input::KeyCode::Left => Self::Left,
            reovim_subsys_input::KeyCode::Right => Self::Right,
            reovim_subsys_input::KeyCode::Home => Self::Home,
            reovim_subsys_input::KeyCode::End => Self::End,
            reovim_subsys_input::KeyCode::PageUp => Self::PageUp,
            reovim_subsys_input::KeyCode::PageDown => Self::PageDown,
            reovim_subsys_input::KeyCode::Backspace => Self::Backspace,
            reovim_subsys_input::KeyCode::Delete => Self::Delete,
            reovim_subsys_input::KeyCode::Insert => Self::Insert,
            reovim_subsys_input::KeyCode::Tab => Self::Tab,
            reovim_subsys_input::KeyCode::BackTab => Self::BackTab,
            reovim_subsys_input::KeyCode::Enter => Self::Enter,
            reovim_subsys_input::KeyCode::Escape => Self::Escape,
            reovim_subsys_input::KeyCode::Null => Self::Null,
            reovim_subsys_input::KeyCode::CapsLock => Self::CapsLock,
            reovim_subsys_input::KeyCode::ScrollLock => Self::ScrollLock,
            reovim_subsys_input::KeyCode::NumLock => Self::NumLock,
            reovim_subsys_input::KeyCode::PrintScreen => Self::PrintScreen,
            reovim_subsys_input::KeyCode::Pause => Self::Pause,
            reovim_subsys_input::KeyCode::Menu => Self::Menu,
            reovim_subsys_input::KeyCode::KeypadBegin => Self::KeypadBegin,
            reovim_subsys_input::KeyCode::MediaPlay => Self::MediaPlay,
            reovim_subsys_input::KeyCode::MediaPause => Self::MediaPause,
            reovim_subsys_input::KeyCode::MediaPlayPause => Self::MediaPlayPause,
            reovim_subsys_input::KeyCode::MediaStop => Self::MediaStop,
            reovim_subsys_input::KeyCode::MediaReverse => Self::MediaReverse,
            reovim_subsys_input::KeyCode::MediaFastForward => Self::MediaFastForward,
            reovim_subsys_input::KeyCode::MediaRewind => Self::MediaRewind,
            reovim_subsys_input::KeyCode::MediaNext => Self::MediaNext,
            reovim_subsys_input::KeyCode::MediaPrevious => Self::MediaPrevious,
            reovim_subsys_input::KeyCode::MediaRecord => Self::MediaRecord,
            reovim_subsys_input::KeyCode::MediaLowerVolume => Self::MediaLowerVolume,
            reovim_subsys_input::KeyCode::MediaRaiseVolume => Self::MediaRaiseVolume,
            reovim_subsys_input::KeyCode::MediaMuteVolume => Self::MediaMuteVolume,
            reovim_subsys_input::KeyCode::LeftShift => Self::LeftShift,
            reovim_subsys_input::KeyCode::RightShift => Self::RightShift,
            reovim_subsys_input::KeyCode::LeftCtrl => Self::LeftCtrl,
            reovim_subsys_input::KeyCode::RightCtrl => Self::RightCtrl,
            reovim_subsys_input::KeyCode::LeftAlt => Self::LeftAlt,
            reovim_subsys_input::KeyCode::RightAlt => Self::RightAlt,
            reovim_subsys_input::KeyCode::LeftSuper => Self::LeftSuper,
            reovim_subsys_input::KeyCode::RightSuper => Self::RightSuper,
            reovim_subsys_input::KeyCode::LeftHyper => Self::LeftHyper,
            reovim_subsys_input::KeyCode::RightHyper => Self::RightHyper,
            reovim_subsys_input::KeyCode::LeftMeta => Self::LeftMeta,
            reovim_subsys_input::KeyCode::RightMeta => Self::RightMeta,
            reovim_subsys_input::KeyCode::IsoLevel3Shift => Self::IsoLevel3Shift,
            reovim_subsys_input::KeyCode::IsoLevel5Shift => Self::IsoLevel5Shift,
        }
    }
}

impl From<KeyCode> for reovim_subsys_input::KeyCode {
    fn from(code: KeyCode) -> Self {
        match code {
            KeyCode::Char(c) => Self::Char(c),
            KeyCode::F(n) => Self::F(n),
            KeyCode::Up => Self::Up,
            KeyCode::Down => Self::Down,
            KeyCode::Left => Self::Left,
            KeyCode::Right => Self::Right,
            KeyCode::Home => Self::Home,
            KeyCode::End => Self::End,
            KeyCode::PageUp => Self::PageUp,
            KeyCode::PageDown => Self::PageDown,
            KeyCode::Backspace => Self::Backspace,
            KeyCode::Delete => Self::Delete,
            KeyCode::Insert => Self::Insert,
            KeyCode::Tab => Self::Tab,
            KeyCode::BackTab => Self::BackTab,
            KeyCode::Enter => Self::Enter,
            KeyCode::Escape => Self::Escape,
            KeyCode::Null => Self::Null,
            KeyCode::CapsLock => Self::CapsLock,
            KeyCode::ScrollLock => Self::ScrollLock,
            KeyCode::NumLock => Self::NumLock,
            KeyCode::PrintScreen => Self::PrintScreen,
            KeyCode::Pause => Self::Pause,
            KeyCode::Menu => Self::Menu,
            KeyCode::KeypadBegin => Self::KeypadBegin,
            KeyCode::MediaPlay => Self::MediaPlay,
            KeyCode::MediaPause => Self::MediaPause,
            KeyCode::MediaPlayPause => Self::MediaPlayPause,
            KeyCode::MediaStop => Self::MediaStop,
            KeyCode::MediaReverse => Self::MediaReverse,
            KeyCode::MediaFastForward => Self::MediaFastForward,
            KeyCode::MediaRewind => Self::MediaRewind,
            KeyCode::MediaNext => Self::MediaNext,
            KeyCode::MediaPrevious => Self::MediaPrevious,
            KeyCode::MediaRecord => Self::MediaRecord,
            KeyCode::MediaLowerVolume => Self::MediaLowerVolume,
            KeyCode::MediaRaiseVolume => Self::MediaRaiseVolume,
            KeyCode::MediaMuteVolume => Self::MediaMuteVolume,
            KeyCode::LeftShift => Self::LeftShift,
            KeyCode::RightShift => Self::RightShift,
            KeyCode::LeftCtrl => Self::LeftCtrl,
            KeyCode::RightCtrl => Self::RightCtrl,
            KeyCode::LeftAlt => Self::LeftAlt,
            KeyCode::RightAlt => Self::RightAlt,
            KeyCode::LeftSuper => Self::LeftSuper,
            KeyCode::RightSuper => Self::RightSuper,
            KeyCode::LeftHyper => Self::LeftHyper,
            KeyCode::RightHyper => Self::RightHyper,
            KeyCode::LeftMeta => Self::LeftMeta,
            KeyCode::RightMeta => Self::RightMeta,
            KeyCode::IsoLevel3Shift => Self::IsoLevel3Shift,
            KeyCode::IsoLevel5Shift => Self::IsoLevel5Shift,
        }
    }
}

impl From<reovim_subsys_input::KeyEventKind> for KeyEventKind {
    fn from(kind: reovim_subsys_input::KeyEventKind) -> Self {
        match kind {
            reovim_subsys_input::KeyEventKind::Press => Self::Press,
            reovim_subsys_input::KeyEventKind::Repeat => Self::Repeat,
            reovim_subsys_input::KeyEventKind::Release => Self::Release,
        }
    }
}

impl From<KeyEventKind> for reovim_subsys_input::KeyEventKind {
    fn from(kind: KeyEventKind) -> Self {
        match kind {
            KeyEventKind::Press => Self::Press,
            KeyEventKind::Repeat => Self::Repeat,
            KeyEventKind::Release => Self::Release,
        }
    }
}

impl From<reovim_subsys_input::KeyEvent> for KeyEvent {
    fn from(event: reovim_subsys_input::KeyEvent) -> Self {
        Self {
            code: event.code.into(),
            modifiers: event.modifiers.into(),
            kind: event.kind.into(),
        }
    }
}

impl From<KeyEvent> for reovim_subsys_input::KeyEvent {
    fn from(event: KeyEvent) -> Self {
        Self {
            code: event.code.into(),
            modifiers: event.modifiers.into(),
            kind: event.kind.into(),
        }
    }
}

impl<T> From<reovim_subsys_input::KeymapResult<T>> for KeymapResult<T> {
    fn from(result: reovim_subsys_input::KeymapResult<T>) -> Self {
        match result {
            reovim_subsys_input::KeymapResult::Match(value) => Self::Match(value),
            reovim_subsys_input::KeymapResult::Prefix => Self::Prefix,
            reovim_subsys_input::KeymapResult::None => Self::None,
        }
    }
}

impl<T> From<KeymapResult<T>> for reovim_subsys_input::KeymapResult<T> {
    fn from(result: KeymapResult<T>) -> Self {
        match result {
            KeymapResult::Match(value) => Self::Match(value),
            KeymapResult::Prefix => Self::Prefix,
            KeymapResult::None => Self::None,
        }
    }
}

impl From<reovim_subsys_input::MouseButton> for MouseButton {
    fn from(button: reovim_subsys_input::MouseButton) -> Self {
        match button {
            reovim_subsys_input::MouseButton::Left => Self::Left,
            reovim_subsys_input::MouseButton::Right => Self::Right,
            reovim_subsys_input::MouseButton::Middle => Self::Middle,
        }
    }
}

impl From<MouseButton> for reovim_subsys_input::MouseButton {
    fn from(button: MouseButton) -> Self {
        match button {
            MouseButton::Left => Self::Left,
            MouseButton::Right => Self::Right,
            MouseButton::Middle => Self::Middle,
        }
    }
}

impl From<reovim_subsys_input::MouseEventKind> for MouseEventKind {
    fn from(kind: reovim_subsys_input::MouseEventKind) -> Self {
        match kind {
            reovim_subsys_input::MouseEventKind::Down(button) => Self::Down(button.into()),
            reovim_subsys_input::MouseEventKind::Up(button) => Self::Up(button.into()),
            reovim_subsys_input::MouseEventKind::Drag(button) => Self::Drag(button.into()),
            reovim_subsys_input::MouseEventKind::Moved => Self::Moved,
            reovim_subsys_input::MouseEventKind::ScrollUp => Self::ScrollUp,
            reovim_subsys_input::MouseEventKind::ScrollDown => Self::ScrollDown,
            reovim_subsys_input::MouseEventKind::ScrollLeft => Self::ScrollLeft,
            reovim_subsys_input::MouseEventKind::ScrollRight => Self::ScrollRight,
        }
    }
}

impl From<MouseEventKind> for reovim_subsys_input::MouseEventKind {
    fn from(kind: MouseEventKind) -> Self {
        match kind {
            MouseEventKind::Down(button) => Self::Down(button.into()),
            MouseEventKind::Up(button) => Self::Up(button.into()),
            MouseEventKind::Drag(button) => Self::Drag(button.into()),
            MouseEventKind::Moved => Self::Moved,
            MouseEventKind::ScrollUp => Self::ScrollUp,
            MouseEventKind::ScrollDown => Self::ScrollDown,
            MouseEventKind::ScrollLeft => Self::ScrollLeft,
            MouseEventKind::ScrollRight => Self::ScrollRight,
        }
    }
}

impl From<reovim_subsys_input::MouseEvent> for MouseEvent {
    fn from(event: reovim_subsys_input::MouseEvent) -> Self {
        Self {
            kind: event.kind.into(),
            column: event.column,
            row: event.row,
            modifiers: event.modifiers.into(),
        }
    }
}

impl From<MouseEvent> for reovim_subsys_input::MouseEvent {
    fn from(event: MouseEvent) -> Self {
        Self {
            kind: event.kind.into(),
            column: event.column,
            row: event.row,
            modifiers: event.modifiers.into(),
        }
    }
}
