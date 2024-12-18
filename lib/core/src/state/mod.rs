use {
    crate::{
        key::Keypress,
        modd::{ExtraMod, Mod},
    },
    std::collections::HashMap,
};

pub struct State {
    pub modd: Mod,
    pub extra_mod: Option<Box<dyn ExtraMod>>,
    pub key_record: Vec<Keypress>,
}

pub struct Mem {
    pub extra_mods: HashMap<String, Box<dyn ExtraMod>>,
}
