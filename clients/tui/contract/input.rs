//! Logical input and Client actions, independent of terminal libraries.

use alloc::string::String;

reovim_bitflags::rv_bitflags! {
    pub struct Modifiers: u8 {
        const CONTROL = 1 << 0;
        const ALT     = 1 << 1;
        const SHIFT   = 2 << 2;
        const META    = 4 << 3;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Code {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Function(u8),
    PageUp,
    PageDown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Key {
    pub code: Code,
    pub modifiers: Modifiers,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Input {
    Key(Key),
    Text(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Observe(u64),
    /// Scalar start/end, replacement text and displayed window origin.
    Insert(u64, u64, String, u64),
    Undo,
    Redo,
    RecoverView,
    /// Release the Client's views; this does not close its Buffer.
    Close,
}
