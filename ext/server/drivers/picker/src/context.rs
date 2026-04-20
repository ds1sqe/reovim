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
#[path = "context_tests.rs"]
mod tests;
