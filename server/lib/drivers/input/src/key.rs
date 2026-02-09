//! Canonical keyboard input types.
//!
//! These types are the source of truth for keyboard input across all platforms.
//! Platform-specific code (arch/unix, arch/wasm, etc.) converts native events
//! to these canonical types.
//!
//! Linux equivalent: `include/uapi/linux/input-event-codes.h`

use bitflags::bitflags;

bitflags! {
    /// Key modifiers (platform-agnostic).
    ///
    /// Supports combinations via bitwise operations.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_driver_input::Modifiers;
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

/// Platform-agnostic key codes.
///
/// Covers all printable ASCII, function keys F1-F24, navigation keys,
/// editing keys, and special keys.
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
    // Media keys (for future platforms)
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
    /// Create a new key event with just a key code (press, no modifiers).
    #[must_use]
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
        }
    }

    /// Create a key event with modifiers (press).
    #[must_use]
    pub const fn with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyEventKind::Press,
        }
    }

    /// Create a key event with full specification.
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
///
/// # Example
///
/// ```
/// use reovim_driver_input::KeymapResult;
///
/// let result: KeymapResult<&str> = KeymapResult::Match("delete_line");
/// assert!(result.is_match());
///
/// let prefix: KeymapResult<&str> = KeymapResult::Prefix;
/// assert!(prefix.is_prefix());
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifiers_bitflags() {
        let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
        assert!(ctrl_shift.contains(Modifiers::CTRL));
        assert!(ctrl_shift.contains(Modifiers::SHIFT));
        assert!(!ctrl_shift.contains(Modifiers::ALT));
        assert!(!ctrl_shift.is_empty());
        assert!(Modifiers::NONE.is_empty());
    }

    #[test]
    fn test_modifiers_all_variants() {
        // Test all modifier flags can be combined
        let all = Modifiers::SHIFT
            | Modifiers::CTRL
            | Modifiers::ALT
            | Modifiers::SUPER
            | Modifiers::HYPER
            | Modifiers::META;
        assert!(all.contains(Modifiers::SHIFT));
        assert!(all.contains(Modifiers::CTRL));
        assert!(all.contains(Modifiers::ALT));
        assert!(all.contains(Modifiers::SUPER));
        assert!(all.contains(Modifiers::HYPER));
        assert!(all.contains(Modifiers::META));
    }

    #[test]
    fn test_key_event_creation() {
        let key = KeyEvent::new(KeyCode::Char('a'));
        assert_eq!(key.code, KeyCode::Char('a'));
        assert_eq!(key.modifiers, Modifiers::NONE);
        assert!(key.is_press());
        assert!(!key.is_release());
        assert!(!key.is_repeat());

        let ctrl_a = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL);
        assert!(ctrl_a.modifiers.contains(Modifiers::CTRL));
        assert!(ctrl_a.is_press());

        let release = KeyEvent::full(KeyCode::Escape, Modifiers::NONE, KeyEventKind::Release);
        assert!(release.is_release());
    }

    #[test]
    fn test_keymap_result_is_match() {
        let match_result: KeymapResult<i32> = KeymapResult::Match(42);
        assert!(match_result.is_match());
        assert!(!match_result.is_prefix());
        assert!(!match_result.is_none());
    }

    #[test]
    fn test_keymap_result_is_prefix() {
        let prefix: KeymapResult<i32> = KeymapResult::Prefix;
        assert!(!prefix.is_match());
        assert!(prefix.is_prefix());
        assert!(!prefix.is_none());
    }

    #[test]
    fn test_keymap_result_is_none() {
        let none: KeymapResult<i32> = KeymapResult::None;
        assert!(!none.is_match());
        assert!(!none.is_prefix());
        assert!(none.is_none());
    }

    #[test]
    fn test_keymap_result_into_option() {
        let match_result: KeymapResult<i32> = KeymapResult::Match(42);
        assert_eq!(match_result.into_option(), Some(42));

        let prefix: KeymapResult<i32> = KeymapResult::Prefix;
        assert_eq!(prefix.into_option(), None);

        let none: KeymapResult<i32> = KeymapResult::None;
        assert_eq!(none.into_option(), None);
    }

    #[test]
    fn test_keymap_result_map() {
        let result: KeymapResult<i32> = KeymapResult::Match(21);
        let mapped = result.map(|x| x * 2);
        assert_eq!(mapped, KeymapResult::Match(42));

        let prefix: KeymapResult<i32> = KeymapResult::Prefix;
        let mapped_prefix: KeymapResult<String> = prefix.map(|x| x.to_string());
        assert!(mapped_prefix.is_prefix());

        let none: KeymapResult<i32> = KeymapResult::None;
        let mapped_none: KeymapResult<String> = none.map(|x| x.to_string());
        assert!(mapped_none.is_none());
    }

    #[test]
    fn test_keymap_result_unwrap() {
        let result: KeymapResult<i32> = KeymapResult::Match(42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    #[should_panic(expected = "Prefix")]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_keymap_result_unwrap_prefix_panics() {
        let prefix: KeymapResult<i32> = KeymapResult::Prefix;
        let _ = prefix.unwrap();
    }

    #[test]
    #[should_panic(expected = "None")]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_keymap_result_unwrap_none_panics() {
        let none: KeymapResult<i32> = KeymapResult::None;
        let _ = none.unwrap();
    }
}
