/// Operator type for operator-pending mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OperatorType {
    #[default]
    Delete,
    Yank,
    Change,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mod {
    Normal,
    Insert(ModExtension),
    Visual(ModExtension),
    Command,
    Explorer,
    /// Explorer input mode (for create/rename/delete/filter)
    ExplorerInput,
    /// Operator-pending mode (waiting for motion after d, y, c, etc.)
    OperatorPending {
        operator: OperatorType,
        count: Option<usize>,
    },
    /// Telescope fuzzy finder mode
    Telescope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModExtension {
    Normal,
    Block,
    Column,
    MultiCusor,
}
