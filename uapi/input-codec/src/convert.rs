//! Conversions between arch types and input-codec typed input vocabulary.

use crate::{KeyCode, KeyEvent, KeyEventKind, Modifiers, MouseButton, MouseEvent, MouseEventKind};

impl From<reovim_arch::Modifiers> for Modifiers {
    fn from(m: reovim_arch::Modifiers) -> Self {
        let mut result = Self::empty();
        if m.contains(reovim_arch::Modifiers::SHIFT) {
            result |= Self::SHIFT;
        }
        if m.contains(reovim_arch::Modifiers::CTRL) {
            result |= Self::CTRL;
        }
        if m.contains(reovim_arch::Modifiers::ALT) {
            result |= Self::ALT;
        }
        if m.contains(reovim_arch::Modifiers::SUPER) {
            result |= Self::SUPER;
        }
        if m.contains(reovim_arch::Modifiers::HYPER) {
            result |= Self::HYPER;
        }
        if m.contains(reovim_arch::Modifiers::META) {
            result |= Self::META;
        }
        result
    }
}

impl From<Modifiers> for reovim_arch::Modifiers {
    fn from(m: Modifiers) -> Self {
        let mut result = Self::NONE;
        if m.contains(Modifiers::SHIFT) {
            result = result.union(Self::SHIFT);
        }
        if m.contains(Modifiers::CTRL) {
            result = result.union(Self::CTRL);
        }
        if m.contains(Modifiers::ALT) {
            result = result.union(Self::ALT);
        }
        if m.contains(Modifiers::SUPER) {
            result = result.union(Self::SUPER);
        }
        if m.contains(Modifiers::HYPER) {
            result = result.union(Self::HYPER);
        }
        if m.contains(Modifiers::META) {
            result = result.union(Self::META);
        }
        result
    }
}

impl From<reovim_arch::KeyCode> for KeyCode {
    fn from(k: reovim_arch::KeyCode) -> Self {
        use reovim_arch::KeyCode as ArchKeyCode;

        match k {
            ArchKeyCode::Char(c) => Self::Char(c),
            ArchKeyCode::F(n) => Self::F(n),
            ArchKeyCode::Backspace => Self::Backspace,
            ArchKeyCode::Enter => Self::Enter,
            ArchKeyCode::Tab => Self::Tab,
            ArchKeyCode::BackTab => Self::BackTab,
            ArchKeyCode::Escape => Self::Escape,
            ArchKeyCode::Up => Self::Up,
            ArchKeyCode::Down => Self::Down,
            ArchKeyCode::Left => Self::Left,
            ArchKeyCode::Right => Self::Right,
            ArchKeyCode::Home => Self::Home,
            ArchKeyCode::End => Self::End,
            ArchKeyCode::PageUp => Self::PageUp,
            ArchKeyCode::PageDown => Self::PageDown,
            ArchKeyCode::Insert => Self::Insert,
            ArchKeyCode::Delete => Self::Delete,
            ArchKeyCode::Null => Self::Null,
            ArchKeyCode::CapsLock => Self::CapsLock,
            ArchKeyCode::ScrollLock => Self::ScrollLock,
            ArchKeyCode::NumLock => Self::NumLock,
            ArchKeyCode::PrintScreen => Self::PrintScreen,
            ArchKeyCode::Pause => Self::Pause,
            ArchKeyCode::Menu => Self::Menu,
            ArchKeyCode::KeypadBegin => Self::KeypadBegin,
            ArchKeyCode::MediaPlay => Self::MediaPlay,
            ArchKeyCode::MediaPause => Self::MediaPause,
            ArchKeyCode::MediaPlayPause => Self::MediaPlayPause,
            ArchKeyCode::MediaStop => Self::MediaStop,
            ArchKeyCode::MediaReverse => Self::MediaReverse,
            ArchKeyCode::MediaFastForward => Self::MediaFastForward,
            ArchKeyCode::MediaRewind => Self::MediaRewind,
            ArchKeyCode::MediaNext => Self::MediaNext,
            ArchKeyCode::MediaPrevious => Self::MediaPrevious,
            ArchKeyCode::MediaRecord => Self::MediaRecord,
            ArchKeyCode::MediaLowerVolume => Self::MediaLowerVolume,
            ArchKeyCode::MediaRaiseVolume => Self::MediaRaiseVolume,
            ArchKeyCode::MediaMuteVolume => Self::MediaMuteVolume,
            ArchKeyCode::LeftShift => Self::LeftShift,
            ArchKeyCode::RightShift => Self::RightShift,
            ArchKeyCode::LeftCtrl => Self::LeftCtrl,
            ArchKeyCode::RightCtrl => Self::RightCtrl,
            ArchKeyCode::LeftAlt => Self::LeftAlt,
            ArchKeyCode::RightAlt => Self::RightAlt,
            ArchKeyCode::LeftSuper => Self::LeftSuper,
            ArchKeyCode::RightSuper => Self::RightSuper,
            ArchKeyCode::LeftHyper => Self::LeftHyper,
            ArchKeyCode::RightHyper => Self::RightHyper,
            ArchKeyCode::LeftMeta => Self::LeftMeta,
            ArchKeyCode::RightMeta => Self::RightMeta,
            ArchKeyCode::IsoLevel3Shift => Self::IsoLevel3Shift,
            ArchKeyCode::IsoLevel5Shift => Self::IsoLevel5Shift,
        }
    }
}

impl From<KeyCode> for reovim_arch::KeyCode {
    fn from(k: KeyCode) -> Self {
        match k {
            KeyCode::Char(c) => Self::Char(c),
            KeyCode::F(n) => Self::F(n),
            KeyCode::Backspace => Self::Backspace,
            KeyCode::Enter => Self::Enter,
            KeyCode::Tab => Self::Tab,
            KeyCode::BackTab => Self::BackTab,
            KeyCode::Escape => Self::Escape,
            KeyCode::Up => Self::Up,
            KeyCode::Down => Self::Down,
            KeyCode::Left => Self::Left,
            KeyCode::Right => Self::Right,
            KeyCode::Home => Self::Home,
            KeyCode::End => Self::End,
            KeyCode::PageUp => Self::PageUp,
            KeyCode::PageDown => Self::PageDown,
            KeyCode::Insert => Self::Insert,
            KeyCode::Delete => Self::Delete,
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

impl From<reovim_arch::KeyEventKind> for KeyEventKind {
    fn from(k: reovim_arch::KeyEventKind) -> Self {
        match k {
            reovim_arch::KeyEventKind::Press => Self::Press,
            reovim_arch::KeyEventKind::Repeat => Self::Repeat,
            reovim_arch::KeyEventKind::Release => Self::Release,
        }
    }
}

impl From<KeyEventKind> for reovim_arch::KeyEventKind {
    fn from(k: KeyEventKind) -> Self {
        match k {
            KeyEventKind::Press => Self::Press,
            KeyEventKind::Repeat => Self::Repeat,
            KeyEventKind::Release => Self::Release,
        }
    }
}

impl From<reovim_arch::KeyEvent> for KeyEvent {
    fn from(e: reovim_arch::KeyEvent) -> Self {
        Self {
            code: e.code.into(),
            modifiers: e.modifiers.into(),
            kind: e.kind.into(),
        }
    }
}

impl From<KeyEvent> for reovim_arch::KeyEvent {
    fn from(e: KeyEvent) -> Self {
        Self {
            code: e.code.into(),
            modifiers: e.modifiers.into(),
            kind: e.kind.into(),
            state: reovim_arch::KeyEventState::NONE,
        }
    }
}

impl From<reovim_arch::MouseButton> for MouseButton {
    fn from(b: reovim_arch::MouseButton) -> Self {
        match b {
            reovim_arch::MouseButton::Left => Self::Left,
            reovim_arch::MouseButton::Right => Self::Right,
            reovim_arch::MouseButton::Middle => Self::Middle,
        }
    }
}

impl From<MouseButton> for reovim_arch::MouseButton {
    fn from(b: MouseButton) -> Self {
        match b {
            MouseButton::Left => Self::Left,
            MouseButton::Right => Self::Right,
            MouseButton::Middle => Self::Middle,
        }
    }
}

impl From<reovim_arch::MouseEventKind> for MouseEventKind {
    fn from(k: reovim_arch::MouseEventKind) -> Self {
        use reovim_arch::MouseEventKind as ArchMouseEventKind;

        match k {
            ArchMouseEventKind::Down(b) => Self::Down(b.into()),
            ArchMouseEventKind::Up(b) => Self::Up(b.into()),
            ArchMouseEventKind::Drag(b) => Self::Drag(b.into()),
            ArchMouseEventKind::Moved => Self::Moved,
            ArchMouseEventKind::ScrollUp => Self::ScrollUp,
            ArchMouseEventKind::ScrollDown => Self::ScrollDown,
            ArchMouseEventKind::ScrollLeft => Self::ScrollLeft,
            ArchMouseEventKind::ScrollRight => Self::ScrollRight,
        }
    }
}

impl From<MouseEventKind> for reovim_arch::MouseEventKind {
    fn from(k: MouseEventKind) -> Self {
        match k {
            MouseEventKind::Down(b) => Self::Down(b.into()),
            MouseEventKind::Up(b) => Self::Up(b.into()),
            MouseEventKind::Drag(b) => Self::Drag(b.into()),
            MouseEventKind::Moved => Self::Moved,
            MouseEventKind::ScrollUp => Self::ScrollUp,
            MouseEventKind::ScrollDown => Self::ScrollDown,
            MouseEventKind::ScrollLeft => Self::ScrollLeft,
            MouseEventKind::ScrollRight => Self::ScrollRight,
        }
    }
}

impl From<reovim_arch::MouseEvent> for MouseEvent {
    fn from(e: reovim_arch::MouseEvent) -> Self {
        Self {
            kind: e.kind.into(),
            column: e.column,
            row: e.row,
            modifiers: e.modifiers.into(),
        }
    }
}

impl From<MouseEvent> for reovim_arch::MouseEvent {
    fn from(e: MouseEvent) -> Self {
        Self {
            kind: e.kind.into(),
            column: e.column,
            row: e.row,
            modifiers: e.modifiers.into(),
        }
    }
}
