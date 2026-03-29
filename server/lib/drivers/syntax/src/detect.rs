//! Language detection from file paths.
//!
//! Maps file extensions to tree-sitter language IDs. Used by consumers
//! that need to create syntax drivers at runtime (e.g., preview highlighting).

use std::path::Path;

/// Detect language ID from a file path's extension.
///
/// Returns the language ID string that can be passed to
/// [`SyntaxDriverFactory::create()`](crate::SyntaxDriverFactory::create)
/// or [`SyntaxFactoryStore::find()`](crate::SyntaxFactoryStore::find).
///
/// Returns `None` for unsupported or missing extensions.
#[must_use]
pub fn language_id_from_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "rs" => Some("rust"),
        "md" | "markdown" => Some("markdown"),
        "py" | "pyi" => Some("python"),
        "go" => Some("go"),
        "c" | "h" => Some("c"),
        "sh" | "bash" => Some("bash"),
        "json" => Some("json"),
        "toml" => Some("toml"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "mts" | "cts" => Some("typescript"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
