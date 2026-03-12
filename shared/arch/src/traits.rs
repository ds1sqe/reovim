//! Platform-agnostic trait definitions.
//!
//! These traits define the interface between the kernel and platform-specific
//! implementations. Linux equivalent: platform-independent kernel headers.
//!
//! All types defined here have NO external dependencies (std only).

use std::{fmt, io, str::FromStr, time::Duration};

// =============================================================================
// Color
// =============================================================================

/// Platform-agnostic color representation.
///
/// Supports ANSI 16 colors, 256-color palette, and 24-bit true color (RGB).
/// This mirrors the color capabilities of modern terminals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Color {
    /// Terminal default (reset color).
    #[default]
    Reset,
    /// Black (ANSI 0).
    Black,
    /// Dark red (ANSI 1).
    DarkRed,
    /// Dark green (ANSI 2).
    DarkGreen,
    /// Dark yellow (ANSI 3).
    DarkYellow,
    /// Dark blue (ANSI 4).
    DarkBlue,
    /// Dark magenta (ANSI 5).
    DarkMagenta,
    /// Dark cyan (ANSI 6).
    DarkCyan,
    /// Grey/light gray (ANSI 7).
    Grey,
    /// Dark grey (ANSI 8).
    DarkGrey,
    /// Red (ANSI 9).
    Red,
    /// Green (ANSI 10).
    Green,
    /// Yellow (ANSI 11).
    Yellow,
    /// Blue (ANSI 12).
    Blue,
    /// Magenta (ANSI 13).
    Magenta,
    /// Cyan (ANSI 14).
    Cyan,
    /// White (ANSI 15).
    White,
    /// 256-color palette value (0-255).
    AnsiValue(u8),
    /// 24-bit true color (RGB).
    Rgb {
        /// Red component (0-255).
        r: u8,
        /// Green component (0-255).
        g: u8,
        /// Blue component (0-255).
        b: u8,
    },
}

// =============================================================================
// Color Parsing and Display
// =============================================================================

/// Error type for color parsing with detailed diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseColorError {
    /// The input that failed to parse.
    pub input: String,
    /// The kind of error.
    pub kind: ParseColorErrorKind,
}

/// Specific error kind for color parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseColorErrorKind {
    /// Invalid hex color length (must be 3 or 6 hex digits after #).
    InvalidHexLength,
    /// Invalid hex digit in color string.
    InvalidHexDigit,
    /// Invalid RGB function format.
    InvalidRgbFormat,
    /// Invalid ANSI index (must be 0-255).
    InvalidAnsiIndex,
    /// Unknown color name.
    UnknownColorName,
}

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ParseColorErrorKind::InvalidHexLength => {
                write!(f, "invalid hex color '{}': must be #rgb or #rrggbb", self.input)
            }
            ParseColorErrorKind::InvalidHexDigit => {
                write!(f, "invalid hex digit in color '{}'", self.input)
            }
            ParseColorErrorKind::InvalidRgbFormat => {
                write!(f, "invalid RGB format '{}': use rgb(r,g,b)", self.input)
            }
            ParseColorErrorKind::InvalidAnsiIndex => {
                write!(f, "invalid ANSI index '{}': must be 0-255", self.input)
            }
            ParseColorErrorKind::UnknownColorName => {
                write!(f, "unknown color name '{}'", self.input)
            }
        }
    }
}

impl std::error::Error for ParseColorError {}

impl FromStr for Color {
    type Err = ParseColorError;

    /// Parse a color from string.
    ///
    /// Supports multiple formats:
    /// - Named colors: `red`, `green`, `blue`, etc. (case-insensitive)
    /// - Hex colors: `#rgb`, `#rrggbb`
    /// - ANSI 256 colors: `ansi:N` where N is 0-255
    /// - RGB function: `rgb(r,g,b)`
    /// - Default/reset: `default`, `reset`
    ///
    /// # Examples
    ///
    /// ```
    /// use reovim_arch::Color;
    ///
    /// assert_eq!("red".parse::<Color>().unwrap(), Color::Red);
    /// assert_eq!("#ff0000".parse::<Color>().unwrap(), Color::Rgb { r: 255, g: 0, b: 0 });
    /// assert_eq!("ansi:196".parse::<Color>().unwrap(), Color::AnsiValue(196));
    /// ```
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        let err = |kind| ParseColorError {
            input: s.to_string(),
            kind,
        };

        // Named colors (case-insensitive)
        match lower.as_str() {
            "default" | "reset" => return Ok(Self::Reset),
            "black" => return Ok(Self::Black),
            "red" => return Ok(Self::Red),
            "green" => return Ok(Self::Green),
            "yellow" => return Ok(Self::Yellow),
            "blue" => return Ok(Self::Blue),
            "magenta" => return Ok(Self::Magenta),
            "cyan" => return Ok(Self::Cyan),
            "white" => return Ok(Self::White),
            "grey" | "gray" => return Ok(Self::Grey),
            "darkgrey" | "darkgray" => return Ok(Self::DarkGrey),
            "darkred" => return Ok(Self::DarkRed),
            "darkgreen" => return Ok(Self::DarkGreen),
            "darkyellow" => return Ok(Self::DarkYellow),
            "darkblue" => return Ok(Self::DarkBlue),
            "darkmagenta" => return Ok(Self::DarkMagenta),
            "darkcyan" => return Ok(Self::DarkCyan),
            _ => {}
        }

        // Hex colors: #rgb or #rrggbb
        if let Some(hex) = s.strip_prefix('#') {
            return Self::parse_hex(hex, s);
        }

        // ANSI 256 colors: ansi:N
        if let Some(n_str) = lower.strip_prefix("ansi:") {
            return n_str
                .parse::<u8>()
                .map(Self::AnsiValue)
                .map_err(|_| err(ParseColorErrorKind::InvalidAnsiIndex));
        }

        // RGB function: rgb(r,g,b)
        if lower.starts_with("rgb(") && lower.ends_with(')') {
            return Self::parse_rgb_func(&lower[4..lower.len() - 1], s);
        }

        Err(err(ParseColorErrorKind::UnknownColorName))
    }
}

impl Color {
    /// Parse a hex color string (without the # prefix).
    fn parse_hex(hex: &str, original: &str) -> Result<Self, ParseColorError> {
        let err = |kind| ParseColorError {
            input: original.to_string(),
            kind,
        };

        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| err(ParseColorErrorKind::InvalidHexDigit))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| err(ParseColorErrorKind::InvalidHexDigit))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| err(ParseColorErrorKind::InvalidHexDigit))?;
                Ok(Self::Rgb { r, g, b })
            }
            3 => {
                // #rgb -> #rrggbb (double each digit)
                let r = u8::from_str_radix(&hex[0..1], 16)
                    .map_err(|_| err(ParseColorErrorKind::InvalidHexDigit))?;
                let g = u8::from_str_radix(&hex[1..2], 16)
                    .map_err(|_| err(ParseColorErrorKind::InvalidHexDigit))?;
                let b = u8::from_str_radix(&hex[2..3], 16)
                    .map_err(|_| err(ParseColorErrorKind::InvalidHexDigit))?;
                Ok(Self::Rgb {
                    r: r * 17,
                    g: g * 17,
                    b: b * 17,
                })
            }
            _ => Err(err(ParseColorErrorKind::InvalidHexLength)),
        }
    }

    /// Parse RGB function content: "r,g,b".
    fn parse_rgb_func(content: &str, original: &str) -> Result<Self, ParseColorError> {
        let err = || ParseColorError {
            input: original.to_string(),
            kind: ParseColorErrorKind::InvalidRgbFormat,
        };

        let parts: Vec<&str> = content.split(',').map(str::trim).collect();
        if parts.len() != 3 {
            return Err(err());
        }

        let r: u8 = parts[0].parse().map_err(|_| err())?;
        let g: u8 = parts[1].parse().map_err(|_| err())?;
        let b: u8 = parts[2].parse().map_err(|_| err())?;

        Ok(Self::Rgb { r, g, b })
    }

    /// Convenience method that returns `Option` instead of `Result`.
    ///
    /// Wraps `FromStr` for compatibility with existing code.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }
}

impl fmt::Display for Color {
    /// Format color as a canonical string representation.
    ///
    /// Output format:
    /// - Named colors: lowercase name (e.g., `red`, `darkblue`)
    /// - RGB colors: `#rrggbb` hex format
    /// - ANSI 256 colors: `ansi:N`
    /// - Reset: `default`
    ///
    /// Guarantees round-trip safety: `color.to_string().parse() == Ok(color)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reset => write!(f, "default"),
            Self::Black => write!(f, "black"),
            Self::Red => write!(f, "red"),
            Self::Green => write!(f, "green"),
            Self::Yellow => write!(f, "yellow"),
            Self::Blue => write!(f, "blue"),
            Self::Magenta => write!(f, "magenta"),
            Self::Cyan => write!(f, "cyan"),
            Self::White => write!(f, "white"),
            Self::Grey => write!(f, "grey"),
            Self::DarkGrey => write!(f, "darkgrey"),
            Self::DarkRed => write!(f, "darkred"),
            Self::DarkGreen => write!(f, "darkgreen"),
            Self::DarkYellow => write!(f, "darkyellow"),
            Self::DarkBlue => write!(f, "darkblue"),
            Self::DarkMagenta => write!(f, "darkmagenta"),
            Self::DarkCyan => write!(f, "darkcyan"),
            Self::Rgb { r, g, b } => write!(f, "#{r:02x}{g:02x}{b:02x}"),
            Self::AnsiValue(n) => write!(f, "ansi:{n}"),
        }
    }
}

// =============================================================================
// Terminal Size
// =============================================================================

/// Terminal size with pixel dimensions for advanced rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalSize {
    /// Number of columns (characters).
    pub cols: u16,
    /// Number of rows (characters).
    pub rows: u16,
    /// Pixel width (if available, 0 otherwise).
    pub pixel_width: u16,
    /// Pixel height (if available, 0 otherwise).
    pub pixel_height: u16,
}

impl TerminalSize {
    /// Create a new terminal size.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    /// Create a new terminal size with pixel dimensions.
    #[must_use]
    pub const fn with_pixels(cols: u16, rows: u16, pixel_width: u16, pixel_height: u16) -> Self {
        Self {
            cols,
            rows,
            pixel_width,
            pixel_height,
        }
    }
}

// =============================================================================
// Key Modifiers
// =============================================================================

/// Key modifiers (bitflags-style, no external dependencies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self(0);
    /// Shift key.
    pub const SHIFT: Self = Self(1 << 0);
    /// Control key.
    pub const CTRL: Self = Self(1 << 1);
    /// Alt/Option key.
    pub const ALT: Self = Self(1 << 2);
    /// Super/Meta/Command key.
    pub const SUPER: Self = Self(1 << 3);
    /// Hyper key (rare).
    pub const HYPER: Self = Self(1 << 4);
    /// Meta key (rare, distinct from Super on some systems).
    pub const META: Self = Self(1 << 5);

    /// Check if a modifier is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine two modifier sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if no modifiers are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Get the raw bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Create from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

// =============================================================================
// Key Codes
// =============================================================================

/// Platform-agnostic key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A character key.
    Char(char),
    /// Function key (F1-F12, or higher on some terminals).
    F(u8),
    /// Backspace key.
    Backspace,
    /// Enter/Return key.
    Enter,
    /// Tab key.
    Tab,
    /// Backspace in some terminals.
    BackTab,
    /// Escape key.
    Escape,
    /// Arrow keys.
    Up,
    Down,
    Left,
    Right,
    /// Navigation keys.
    Home,
    End,
    PageUp,
    PageDown,
    /// Editing keys.
    Insert,
    Delete,
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
    /// Menu key (if reported).
    Menu,
    /// Keypad begin (center key on keypad).
    KeypadBegin,
    /// Media keys.
    MediaPlay,
    MediaPause,
    MediaPlayPause,
    MediaStop,
    MediaReverse,
    MediaFastForward,
    MediaRewind,
    MediaNext,
    MediaPrevious,
    MediaRecord,
    MediaLowerVolume,
    MediaRaiseVolume,
    MediaMuteVolume,
    /// Modifier keys (if reported as separate events).
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    LeftSuper,
    RightSuper,
    LeftHyper,
    RightHyper,
    LeftMeta,
    RightMeta,
    IsoLevel3Shift,
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

/// Key event state (for enhanced keyboard protocols).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyEventState(u8);

impl KeyEventState {
    /// No state.
    pub const NONE: Self = Self(0);
    /// Key is from keypad.
    pub const KEYPAD: Self = Self(1 << 0);
    /// Caps lock is active.
    pub const CAPS_LOCK: Self = Self(1 << 1);
    /// Num lock is active.
    pub const NUM_LOCK: Self = Self(1 << 2);

    /// Check if a state flag is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine two state sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if no state flags are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Key event with modifiers and kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// The key code.
    pub code: KeyCode,
    /// Active modifiers.
    pub modifiers: Modifiers,
    /// Event kind (press, repeat, release).
    pub kind: KeyEventKind,
    /// Additional state (keypad, caps lock, etc.).
    pub state: KeyEventState,
}

impl KeyEvent {
    /// Create a new key event with just a key code.
    #[must_use]
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// Create a key event with modifiers.
    #[must_use]
    pub const fn with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }
}

// =============================================================================
// Mouse Events
// =============================================================================

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button (wheel click).
    Middle,
}

/// Mouse event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseEventKind {
    /// Button pressed down.
    Down(MouseButton),
    /// Button released.
    Up(MouseButton),
    /// Mouse dragged with button held.
    Drag(MouseButton),
    /// Mouse moved (without button).
    Moved,
    /// Scroll up.
    ScrollUp,
    /// Scroll down.
    ScrollDown,
    /// Scroll left (horizontal).
    ScrollLeft,
    /// Scroll right (horizontal).
    ScrollRight,
}

/// Mouse event with position and modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    /// The event kind.
    pub kind: MouseEventKind,
    /// Column (0-based).
    pub column: u16,
    /// Row (0-based).
    pub row: u16,
    /// Active modifiers.
    pub modifiers: Modifiers,
}

// =============================================================================
// Input Events
// =============================================================================

/// Union of all input events from the terminal.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Keyboard event.
    Key(KeyEvent),
    /// Mouse event.
    Mouse(MouseEvent),
    /// Terminal resize event.
    Resize(TerminalSize),
    /// Terminal gained focus.
    FocusGained,
    /// Terminal lost focus.
    FocusLost,
    /// Bracketed paste content.
    Paste(String),
}

// =============================================================================
// Clear Type
// =============================================================================

/// Terminal clear type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearType {
    /// Clear entire screen.
    All,
    /// Clear from cursor to end of screen.
    FromCursorDown,
    /// Clear from cursor to start of screen.
    FromCursorUp,
    /// Clear current line.
    CurrentLine,
    /// Clear from cursor to end of line.
    UntilNewLine,
    /// Purge scrollback buffer (if supported).
    Purge,
}

// =============================================================================
// Raw Mode Guard
// =============================================================================

/// RAII guard for raw mode - restores terminal on drop.
///
/// When this guard is dropped, it automatically restores the terminal
/// to its previous state (typically cooked mode).
pub struct RawModeGuard {
    restore_fn: Option<Box<dyn FnOnce() -> io::Result<()> + Send>>,
}

impl RawModeGuard {
    /// Create a new raw mode guard with a restore function.
    pub fn new<F>(restore_fn: F) -> Self
    where
        F: FnOnce() -> io::Result<()> + Send + 'static,
    {
        Self {
            restore_fn: Some(Box::new(restore_fn)),
        }
    }

    /// Create a disabled guard that does nothing on drop.
    #[must_use]
    pub fn disabled() -> Self {
        Self { restore_fn: None }
    }

    /// Take the restore function without running it.
    ///
    /// After calling this, the guard will not restore on drop.
    pub fn take(&mut self) -> Option<Box<dyn FnOnce() -> io::Result<()> + Send>> {
        self.restore_fn.take()
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if let Some(f) = self.restore_fn.take() {
            // Ignore errors in drop - can't propagate them
            let _ = f();
        }
    }
}

// =============================================================================
// Terminal Trait
// =============================================================================

/// Terminal backend abstraction.
///
/// Provides size queries, raw mode control, cursor manipulation, and screen management.
/// This trait is the primary interface for terminal I/O operations.
#[allow(clippy::missing_errors_doc)]
pub trait Terminal: Send + Sync {
    /// Get terminal dimensions.
    ///
    /// Returns the current terminal size in characters and optionally pixels.
    fn size(&self) -> io::Result<TerminalSize>;

    /// Enable raw mode.
    ///
    /// In raw mode:
    /// - Input is not line-buffered
    /// - Echo is disabled
    /// - Special key combinations are passed through (Ctrl+C, etc.)
    ///
    /// Returns a guard that restores the terminal on drop.
    fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard>;

    /// Check if keyboard enhancement protocol is supported.
    ///
    /// The keyboard enhancement protocol (kitty keyboard protocol) allows
    /// distinguishing keys like Ctrl+I from Tab.
    fn supports_keyboard_enhancement(&self) -> bool;

    /// Enable keyboard enhancement protocol if supported.
    fn enable_keyboard_enhancement(&mut self) -> io::Result<()>;

    /// Disable keyboard enhancement protocol.
    fn disable_keyboard_enhancement(&mut self) -> io::Result<()>;

    /// Enter alternate screen buffer.
    ///
    /// The alternate screen isolates the editor UI from the shell.
    fn enter_alternate_screen(&mut self) -> io::Result<()>;

    /// Leave alternate screen buffer.
    ///
    /// Restores the original shell content.
    fn leave_alternate_screen(&mut self) -> io::Result<()>;

    /// Hide the cursor.
    fn hide_cursor(&mut self) -> io::Result<()>;

    /// Show the cursor.
    fn show_cursor(&mut self) -> io::Result<()>;

    /// Move cursor to position (0-based).
    fn move_cursor(&mut self, col: u16, row: u16) -> io::Result<()>;

    /// Clear terminal.
    fn clear(&mut self, clear_type: ClearType) -> io::Result<()>;

    /// Enable mouse capture.
    ///
    /// Mouse events will be reported as [`InputEvent::Mouse`].
    fn enable_mouse_capture(&mut self) -> io::Result<()>;

    /// Disable mouse capture.
    fn disable_mouse_capture(&mut self) -> io::Result<()>;

    /// Write bytes to the terminal.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;

    /// Write a string to the terminal.
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.write(s.as_bytes())?;
        Ok(())
    }

    /// Flush output buffer.
    fn flush(&mut self) -> io::Result<()>;
}

// =============================================================================
// Input Source Trait
// =============================================================================

/// Input source abstraction.
///
/// Provides a blocking API for reading terminal input events.
/// This is the low-level interface; higher layers may wrap this with async.
#[allow(clippy::missing_errors_doc)]
pub trait InputSource: Send {
    /// Poll for input with timeout.
    ///
    /// Returns `true` if an event is available, `false` if timeout elapsed.
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;

    /// Read next input event.
    ///
    /// This is a blocking call. Use `poll` first to check for availability.
    fn read_event(&mut self) -> io::Result<InputEvent>;

    /// Drain all pending events.
    ///
    /// Returns a vector of all currently queued events.
    fn drain(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::new();
        while self.poll(Duration::ZERO).unwrap_or(false) {
            if let Ok(event) = self.read_event() {
                events.push(event);
            }
        }
        events
    }
}

// =============================================================================
// Signal Handler Trait
// =============================================================================

/// Signal handler abstraction.
///
/// Provides callbacks for OS signals. Note that on Unix with crossterm,
/// resize signals (SIGWINCH) are typically delivered as [`InputEvent::Resize`]
/// through the input source rather than through this handler.
pub trait SignalHandler: Send + Sync {
    /// Register handler for terminal resize.
    ///
    /// Note: On most platforms with crossterm, resize is delivered via
    /// [`InputEvent::Resize`] instead of through this handler.
    fn on_resize(&mut self, handler: Box<dyn Fn(TerminalSize) + Send + Sync>);

    /// Register handler for interrupt (Ctrl+C / SIGINT).
    fn on_interrupt(&mut self, handler: Box<dyn Fn() + Send + Sync>);

    /// Register handler for suspend (Ctrl+Z / SIGTSTP).
    fn on_suspend(&mut self, handler: Box<dyn Fn() + Send + Sync>);
}

#[cfg(test)]
#[path = "traits_tests.rs"]
mod tests;
