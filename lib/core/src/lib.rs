use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Modifier {
    Shift,
    Control,
    Meta,
    Super,
}

pub enum Mod {
    Normal,
    Insert(ModExtension),
    Visual(ModExtension),
    Command,
}

pub enum ModExtension {
    Normal,
    Block,
    Column,
    MultiCusor,
}

pub trait ExtraMod {
    fn name(&self) -> &str;
}

pub struct TelescopeMod {}
//
// pub enum ExtraMod {
//     None,
//     Telescope,
//     Git,
//     Lsp,
// }

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

pub struct Action {}

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

pub struct State {
    pub modd: Mod,
    pub extra_mod: Option<Box<dyn ExtraMod>>,
    pub key_record: Vec<Keypress>,
}

pub struct Mem {
    pub extra_mods: HashMap<String, Box<dyn ExtraMod>>,
}
