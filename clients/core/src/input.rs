//! The Client-side key. Raw keys never cross the wire; a consumer turns
//! them into calls.

/// One key as the terminal gave it, already decoded by the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    /// Control and a letter, lower case.
    Ctrl(char),
    Esc,
}
