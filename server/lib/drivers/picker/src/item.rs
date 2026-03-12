use std::path::PathBuf;

/// A single item in picker results.
#[derive(Debug, Clone)]
pub struct PickerItem {
    /// Primary display text (used for fuzzy matching).
    pub display: String,
    /// Secondary detail text (shown grayed, e.g. relative path).
    pub detail: Option<String>,
    /// Opaque data payload for action resolution.
    pub data: PickerData,
    /// Icon/indicator character (e.g. file type icon).
    pub icon: Option<char>,
}

/// Data associated with a picker item, determines action on selection.
#[derive(Debug, Clone)]
pub enum PickerData {
    /// A file path to open.
    FilePath(PathBuf),
    /// A buffer ID to switch to.
    BufferId(usize),
    /// A command to execute (qualified "module:command" string).
    Command(String),
    /// A location in a file (e.g. grep match, diagnostic, reference).
    GotoLocation {
        path: PathBuf,
        line: usize,
        col: usize,
    },
    /// Arbitrary string data.
    Text(String),
}

#[cfg(test)]
#[path = "item_tests.rs"]
mod tests;
