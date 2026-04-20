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

/// Get a Nerd Font icon char for a file extension.
///
/// Returns `None` for unrecognized extensions.
#[must_use]
pub fn file_type_icon(extension: &str) -> Option<char> {
    match extension {
        "rs" => Some('\u{e7a8}'),                    //
        "py" | "pyi" => Some('\u{e73c}'),            //
        "js" | "mjs" | "cjs" => Some('\u{e781}'),    //
        "ts" | "mts" | "cts" => Some('\u{e628}'),    //
        "go" => Some('\u{e626}'),                    //
        "c" | "h" => Some('\u{e61e}'),               //
        "cpp" | "hpp" => Some('\u{e61d}'),           //
        "sh" | "bash" | "zsh" => Some('\u{e795}'),   //
        "html" | "htm" => Some('\u{e736}'),          //
        "css" | "scss" => Some('\u{e749}'),          //
        "md" | "markdown" => Some('\u{e73e}'),       //
        "json" => Some('\u{e60b}'),                  //
        "toml" | "yaml" | "yml" => Some('\u{e615}'), //
        "lua" => Some('\u{e620}'),                   //
        "java" => Some('\u{e738}'),                  //
        "rb" => Some('\u{e739}'),                    //
        "xml" => Some('\u{e619}'),                   //
        _ => None,
    }
}

/// Get icon char from a file path string by extracting the extension.
#[must_use]
pub fn icon_for_path(path: &str) -> Option<char> {
    let ext = path.rsplit_once('.')?.1;
    file_type_icon(ext)
}

#[cfg(test)]
#[path = "item_tests.rs"]
mod tests;
