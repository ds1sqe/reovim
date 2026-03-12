use std::path::PathBuf;

/// Action to perform when a picker item is selected.
#[derive(Debug, Clone)]
pub enum PickerAction {
    /// Open a file at the given path.
    OpenFile(PathBuf),
    /// Switch to a buffer by ID.
    SwitchBuffer(usize),
    /// Execute a command by qualified name.
    ExecuteCommand(String),
    /// Go to a specific location in a file.
    GotoLocation {
        path: PathBuf,
        line: usize,
        col: usize,
    },
    /// Close picker without action.
    Close,
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
