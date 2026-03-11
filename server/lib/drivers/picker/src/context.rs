use std::path::PathBuf;

/// Context available to picker implementations.
#[derive(Debug, Clone)]
pub struct PickerContext {
    /// Working directory for file-based pickers.
    pub cwd: PathBuf,
    /// Current query text.
    pub query: String,
    /// Available buffers (for buffer picker).
    pub buffers: Vec<BufferInfo>,
    /// Available commands (for command palette).
    pub commands: Vec<CommandInfo>,
    /// Available options (for option picker).
    pub options: Vec<OptionInfo>,
}

/// Information about an open buffer.
#[derive(Debug, Clone)]
pub struct BufferInfo {
    /// Buffer identifier.
    pub id: usize,
    /// Buffer display name (file name or `[scratch]`).
    pub name: String,
    /// Whether the buffer has unsaved modifications.
    pub modified: bool,
}

/// Information about a registered command.
#[derive(Debug, Clone)]
pub struct CommandInfo {
    /// Fully qualified command name (e.g. "editor:save").
    pub qualified_name: String,
    /// Human-readable description.
    pub description: String,
}

/// Information about a registered option.
#[derive(Debug, Clone)]
pub struct OptionInfo {
    /// Option name (e.g. "number", "`picker_height`").
    pub name: String,
    /// Short alias if any (e.g. "nu" for "number").
    pub short_form: Option<String>,
    /// Human-readable description.
    pub description: String,
    /// Type name: "bool", "integer", "string", or "choice".
    pub type_name: String,
    /// Display string of the current value.
    pub current_value: String,
    /// Display string of the default value.
    pub default_value: String,
    /// Human-readable constraint description (e.g. "3..50").
    pub constraint: Option<String>,
    /// Scope: "global", "buffer", or "window".
    pub scope: String,
    /// Owning module ID (e.g. "microscope").
    pub owner: Option<String>,
    /// Available choices for choice-type options.
    pub choices: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_construction() {
        let ctx = PickerContext {
            cwd: PathBuf::from("/home/user/project"),
            query: "main".to_owned(),
            buffers: vec![],
            commands: vec![],
            options: vec![],
        };
        assert_eq!(ctx.cwd, PathBuf::from("/home/user/project"));
        assert_eq!(ctx.query, "main");
        assert!(ctx.buffers.is_empty());
        assert!(ctx.commands.is_empty());
    }

    #[test]
    fn context_with_buffers() {
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![
                BufferInfo {
                    id: 1,
                    name: "main.rs".to_owned(),
                    modified: false,
                },
                BufferInfo {
                    id: 2,
                    name: "lib.rs".to_owned(),
                    modified: true,
                },
            ],
            commands: vec![],
            options: vec![],
        };
        assert_eq!(ctx.buffers.len(), 2);
        assert_eq!(ctx.buffers[0].name, "main.rs");
        assert!(!ctx.buffers[0].modified);
        assert!(ctx.buffers[1].modified);
    }

    #[test]
    fn context_with_commands() {
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![CommandInfo {
                qualified_name: "editor:save".to_owned(),
                description: "Save file".to_owned(),
            }],
            options: vec![],
        };
        assert_eq!(ctx.commands.len(), 1);
        assert_eq!(ctx.commands[0].qualified_name, "editor:save");
    }

    #[test]
    fn context_clone() {
        let ctx = PickerContext {
            cwd: PathBuf::from("/tmp"),
            query: "test".to_owned(),
            buffers: vec![BufferInfo {
                id: 1,
                name: "a.rs".to_owned(),
                modified: false,
            }],
            commands: vec![],
            options: vec![],
        };
        #[allow(clippy::redundant_clone)]
        let cloned = ctx.clone();
        assert_eq!(cloned.query, "test");
        assert_eq!(cloned.buffers.len(), 1);
    }

    #[test]
    fn context_debug() {
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
            options: vec![],
        };
        let debug = format!("{ctx:?}");
        assert!(debug.contains("PickerContext"));
    }

    #[test]
    fn buffer_info_debug_clone() {
        let info = BufferInfo {
            id: 5,
            name: "test.rs".to_owned(),
            modified: true,
        };
        let cloned = info.clone();
        assert_eq!(cloned.id, 5);
        let debug = format!("{info:?}");
        assert!(debug.contains("BufferInfo"));
    }

    #[test]
    fn command_info_debug_clone() {
        let info = CommandInfo {
            qualified_name: "vim:delete".to_owned(),
            description: "Delete text".to_owned(),
        };
        let cloned = info.clone();
        assert_eq!(cloned.qualified_name, "vim:delete");
        let debug = format!("{info:?}");
        assert!(debug.contains("CommandInfo"));
    }

    #[test]
    fn option_info_debug_clone() {
        let info = OptionInfo {
            name: "number".to_owned(),
            short_form: Some("nu".to_owned()),
            description: "Show line numbers".to_owned(),
            type_name: "bool".to_owned(),
            current_value: "false".to_owned(),
            default_value: "false".to_owned(),
            constraint: None,
            scope: "window".to_owned(),
            owner: Some("options".to_owned()),
            choices: None,
        };
        let cloned = info.clone();
        assert_eq!(cloned.name, "number");
        assert_eq!(cloned.short_form.as_deref(), Some("nu"));
        let debug = format!("{info:?}");
        assert!(debug.contains("OptionInfo"));
    }

    #[test]
    fn context_with_options() {
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
            options: vec![OptionInfo {
                name: "number".to_owned(),
                short_form: None,
                description: "Line numbers".to_owned(),
                type_name: "bool".to_owned(),
                current_value: "true".to_owned(),
                default_value: "false".to_owned(),
                constraint: None,
                scope: "window".to_owned(),
                owner: None,
                choices: None,
            }],
        };
        assert_eq!(ctx.options.len(), 1);
        assert_eq!(ctx.options[0].name, "number");
    }
}
