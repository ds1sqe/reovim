#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mod {
    Normal,
    Insert(ModExtension),
    Visual(ModExtension),
    Command,
    Explorer,
    /// Explorer input mode (for create/rename/delete/filter)
    ExplorerInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModExtension {
    Normal,
    Block,
    Column,
    MultiCusor,
}
