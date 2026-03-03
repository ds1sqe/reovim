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
mod tests {
    use super::*;

    #[test]
    fn action_open_file() {
        let action = PickerAction::OpenFile(PathBuf::from("src/main.rs"));
        let debug = format!("{action:?}");
        assert!(debug.contains("OpenFile"));
    }

    #[test]
    fn action_switch_buffer() {
        let action = PickerAction::SwitchBuffer(3);
        let debug = format!("{action:?}");
        assert!(debug.contains("SwitchBuffer"));
    }

    #[test]
    fn action_execute_command() {
        let action = PickerAction::ExecuteCommand("editor:save".to_owned());
        let debug = format!("{action:?}");
        assert!(debug.contains("ExecuteCommand"));
    }

    #[test]
    fn action_goto_location() {
        let action = PickerAction::GotoLocation {
            path: PathBuf::from("lib.rs"),
            line: 42,
            col: 10,
        };
        let debug = format!("{action:?}");
        assert!(debug.contains("GotoLocation"));
    }

    #[test]
    fn action_close() {
        let action = PickerAction::Close;
        let debug = format!("{action:?}");
        assert!(debug.contains("Close"));
    }

    #[test]
    fn action_clone() {
        let action = PickerAction::GotoLocation {
            path: PathBuf::from("a.rs"),
            line: 1,
            col: 2,
        };
        #[allow(clippy::redundant_clone)]
        let cloned = action.clone();
        let debug = format!("{cloned:?}");
        assert!(debug.contains("GotoLocation"));
    }
}
