//! TUI keyboard input vocabulary.
//!
//! Owns the typed key/mouse vocabulary for terminal platforms.  This module
//! was split out of `reovim-input-codec` (the closed UAPI mechanism) so that
//! platform vocabulary does not pollute the generic envelope crate.

use bitflags::bitflags;

bitflags! {
    /// Key modifiers (platform-agnostic).
    ///
    /// Supports combinations via bitwise operations.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_codec_tui_input::Modifiers;
    ///
    /// let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
    /// assert!(ctrl_shift.contains(Modifiers::CTRL));
    /// assert!(ctrl_shift.contains(Modifiers::SHIFT));
    /// assert!(!ctrl_shift.contains(Modifiers::ALT));
    /// ```
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Modifiers: u8 {
        /// No modifiers.
        const NONE  = 0;
        /// Shift key.
        const SHIFT = 1 << 0;
        /// Control key.
        const CTRL  = 1 << 1;
        /// Alt/Option key.
        const ALT   = 1 << 2;
        /// Super/Meta/Command key.
        const SUPER = 1 << 3;
        /// Hyper key (rare).
        const HYPER = 1 << 4;
        /// Meta key (distinct from Super on some systems).
        const META  = 1 << 5;
    }
}

/// Platform-agnostic key codes for TUI terminals.
///
/// Covers all printable ASCII/Unicode, function keys F1-F24, navigation keys,
/// editing keys, media keys, per-side modifier keycodes, and ISO level shift keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // =========================================================================
    // Printable characters
    // =========================================================================
    /// A character key (covers all printable ASCII and Unicode).
    Char(char),

    // =========================================================================
    // Function keys (F1-F24)
    // =========================================================================
    /// Function key (F1-F24, stored as 1-24).
    F(u8),

    // =========================================================================
    // Navigation keys
    // =========================================================================
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,

    // =========================================================================
    // Editing keys
    // =========================================================================
    /// Backspace key.
    Backspace,
    /// Delete key.
    Delete,
    /// Insert key.
    Insert,
    /// Tab key.
    Tab,
    /// Shift+Tab (backtab).
    BackTab,
    /// Enter/Return key.
    Enter,
    /// Escape key.
    Escape,

    // =========================================================================
    // Special keys
    // =========================================================================
    /// Null character (Ctrl+@).
    Null,
    /// Caps lock (if reported).
    CapsLock,
    /// Scroll lock (if reported).
    ScrollLock,
    /// Num lock (if reported).
    NumLock,
    /// Print screen (if reported).
    PrintScreen,
    /// Pause key (if reported).
    Pause,
    /// Menu/Application key (if reported).
    Menu,
    /// Keypad begin (center key on keypad).
    KeypadBegin,

    // =========================================================================
    // Media keys
    // =========================================================================
    /// Media play.
    MediaPlay,
    /// Media pause.
    MediaPause,
    /// Media play/pause toggle.
    MediaPlayPause,
    /// Media stop.
    MediaStop,
    /// Media reverse.
    MediaReverse,
    /// Media fast forward.
    MediaFastForward,
    /// Media rewind.
    MediaRewind,
    /// Media next track.
    MediaNext,
    /// Media previous track.
    MediaPrevious,
    /// Media record.
    MediaRecord,
    /// Media lower volume.
    MediaLowerVolume,
    /// Media raise volume.
    MediaRaiseVolume,
    /// Media mute volume.
    MediaMuteVolume,

    // =========================================================================
    // Modifier keys (when reported as separate events)
    // =========================================================================
    /// Left shift key.
    LeftShift,
    /// Right shift key.
    RightShift,
    /// Left control key.
    LeftCtrl,
    /// Right control key.
    RightCtrl,
    /// Left alt key.
    LeftAlt,
    /// Right alt key.
    RightAlt,
    /// Left super/meta key.
    LeftSuper,
    /// Right super/meta key.
    RightSuper,
    /// Left hyper key.
    LeftHyper,
    /// Right hyper key.
    RightHyper,
    /// Left meta key.
    LeftMeta,
    /// Right meta key.
    RightMeta,
    /// ISO Level 3 Shift (`AltGr` on some keyboards).
    IsoLevel3Shift,
    /// ISO Level 5 Shift.
    IsoLevel5Shift,
}

/// Key event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyEventKind {
    /// Key was pressed.
    #[default]
    Press,
    /// Key is being held (repeat).
    Repeat,
    /// Key was released.
    Release,
}

/// Complete key event.
///
/// Represents a keyboard event with key code, modifiers, and event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// The key code.
    pub code: KeyCode,
    /// Active modifiers.
    pub modifiers: Modifiers,
    /// Event kind (press, repeat, release).
    pub kind: KeyEventKind,
}

impl KeyEvent {
    /// Create a key press event with no modifiers.
    #[must_use]
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        }
    }

    /// Create a key event with modifiers.
    #[must_use]
    pub const fn with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyEventKind::Press,
        }
    }

    /// Create a key event with explicit kind and modifiers.
    #[must_use]
    pub const fn full(code: KeyCode, modifiers: Modifiers, kind: KeyEventKind) -> Self {
        Self {
            code,
            modifiers,
            kind,
        }
    }

    /// Check if this is a press event.
    #[must_use]
    pub const fn is_press(&self) -> bool {
        matches!(self.kind, KeyEventKind::Press)
    }

    /// Check if this is a release event.
    #[must_use]
    pub const fn is_release(&self) -> bool {
        matches!(self.kind, KeyEventKind::Release)
    }

    /// Check if this is a repeat event.
    #[must_use]
    pub const fn is_repeat(&self) -> bool {
        matches!(self.kind, KeyEventKind::Repeat)
    }
}

/// Result of keymap lookup.
///
/// Supports multi-key sequences like `gg`, `<C-w>h`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapResult<T> {
    /// Full match: the key sequence maps to an action.
    Match(T),
    /// Partial match: the key sequence is a prefix of one or more bindings.
    Prefix,
    /// No match: the key sequence does not match any binding.
    None,
}

impl<T> KeymapResult<T> {
    /// Returns `true` if this is a full match.
    #[must_use]
    pub const fn is_match(&self) -> bool {
        matches!(self, Self::Match(_))
    }

    /// Returns `true` if this is a prefix.
    #[must_use]
    pub const fn is_prefix(&self) -> bool {
        matches!(self, Self::Prefix)
    }

    /// Returns `true` if there is no match.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Convert to Option, returning Some only for Match.
    #[must_use]
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Match(action) => Some(action),
            Self::Prefix | Self::None => None,
        }
    }

    /// Map the action type.
    #[must_use]
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> KeymapResult<U> {
        match self {
            Self::Match(action) => KeymapResult::Match(f(action)),
            Self::Prefix => KeymapResult::Prefix,
            Self::None => KeymapResult::None,
        }
    }

    /// Returns the contained Match value, consuming self.
    ///
    /// # Panics
    ///
    /// Panics if the value is not Match.
    #[must_use]
    pub fn unwrap(self) -> T {
        match self {
            Self::Match(action) => action,
            Self::Prefix => panic!("called `KeymapResult::unwrap()` on a `Prefix` value"),
            Self::None => panic!("called `KeymapResult::unwrap()` on a `None` value"),
        }
    }
}

/// Encode a `KeyCode` to a `u32` for wire format.
///
/// Encoding table (hex):
/// ```text
/// 0x0000        Null
/// 0x0001        Backspace
/// 0x0002        Enter
/// 0x0003        Left
/// 0x0004        Right
/// 0x0005        Up
/// 0x0006        Down
/// 0x0007        Home
/// 0x0008        End
/// 0x0009        PageUp
/// 0x000A        PageDown
/// 0x000B        Tab
/// 0x000C        BackTab
/// 0x000D        Delete
/// 0x000E        Insert
/// 0x000F        Escape
/// 0x0010        CapsLock
/// 0x0011        ScrollLock
/// 0x0012        NumLock
/// 0x0013        PrintScreen
/// 0x0014        Pause
/// 0x0015        Menu
/// 0x0016        KeypadBegin
/// 0x0020-0x002C Media keys (play..mute)
/// 0x0030-0x003D Modifier keys (LeftShift..IsoLevel5Shift)
/// 0x0100_0000+  Char(c) — bit 24 is the flag; low 21 bits carry the Unicode codepoint
///                         (covers U+0000 to U+10FFFF, the full Unicode range)
/// 0x0200_0000+  F(n)    — bit 25 is the flag; low 8 bits carry function number (1-24)
/// ```
#[must_use]
pub fn keycode_to_u32(code: &KeyCode) -> u32 {
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
        // Char: bit 24 (0x0100_0000) as flag, low 21 bits carry the Unicode codepoint.
        // This covers all of Unicode (U+0000 to U+10FFFF, which fits in 21 bits).
        KeyCode::Char(c) => 0x0100_0000 | u32::from(*c),
        // F key: bit 25 (0x0200_0000) as flag, low 8 bits carry function number.
        KeyCode::F(n) => 0x0200_0000 | u32::from(*n),
    }
}

/// Decode a `u32` back to a `KeyCode`.
///
/// Returns `KeyCode::Null` for unknown values.
#[must_use]
pub fn u32_to_keycode(value: u32) -> KeyCode {
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
        v if v & 0x0200_0000 != 0 => {
            #[allow(clippy::cast_possible_truncation)]
            KeyCode::F((v & 0xFF) as u8)
        }
        // Char: bit 24 set, low 21 bits are the Unicode codepoint.
        v if v & 0x0100_0000 != 0 => {
            let codepoint = v & 0x001F_FFFF; // low 21 bits
            let c = char::from_u32(codepoint).unwrap_or('\0');
            KeyCode::Char(c)
        }
        _ => KeyCode::Null,
    }
}
