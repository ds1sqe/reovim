#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mod {
    Normal,
    Insert(ModExtension),
    Visual(ModExtension),
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
