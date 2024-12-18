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
