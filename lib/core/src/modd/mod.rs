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
