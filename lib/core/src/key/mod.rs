#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Modifier {
    Shift,
    Control,
    Meta,
    Super,
}

#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Key {
    Meta, // enter, backspace, etc
    Ascii,
    F, // f1, f2 ...
}

#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Keypress {
    pub modi: Option<Modifier>,
    pub key: Key,
}
