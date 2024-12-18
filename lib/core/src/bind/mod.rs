use {
    crate::{
        key::Keypress,
        modd::{ExtraMod, Mod},
    },
    std::collections::{HashMap, HashSet},
};

pub struct KeyBind {
    pub keys: Vec<Keypress>,
    pub on: HashSet<Mod>,
    pub on_extra: HashSet<Box<dyn ExtraMod>>,
}

pub struct KeyBindSet {
    pub privileged: bool,
    pub on: HashSet<String>, // mod
    pub keys: String,
}

pub struct KeyMapInner {
    next: Option<HashMap<Keypress, KeyMapInner>>,
    // ... action.
}

/// trie for keymap
pub struct KeyMap {
    pub imap: HashMap<String, KeyMapInner>,
    pub nmap: HashMap<String, KeyMapInner>,
    pub cmap: HashMap<String, KeyMapInner>,
    pub vmap: HashMap<String, KeyMapInner>,
    pub extra: HashMap<String, KeyMapInner>,
}
