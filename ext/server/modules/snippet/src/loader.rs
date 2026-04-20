//! JSON snippet file loader (#136).
//!
//! Loads snippet definitions from VSCode-compatible JSON files.
//!
//! # File Format
//!
//! ```json
//! {
//!   "snippet_name": {
//!     "prefix": "trigger",
//!     "body": "fn ${1:name}() {\n\t$0\n}",
//!     "description": "optional description"
//!   }
//! }
//! ```
//!
//! The `body` field can be a string or an array of strings (joined with `\n`).
//!
//! # Directory Layout
//!
//! ```text
//! ~/.local/share/reovim/snippets/
//! ├── rust.json
//! ├── python.json
//! └── global.json
//! ```

use std::{collections::HashMap, fmt, path::Path};

use crate::provider::{SnippetDefinition, SnippetProvider};

/// Error loading snippet files.
#[derive(Debug)]
pub enum LoadError {
    /// I/O error reading a file.
    Io(std::io::Error),
    /// JSON parsing error.
    Json(serde_json::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "snippet file I/O error: {e}"),
            Self::Json(e) => write!(f, "snippet JSON error: {e}"),
        }
    }
}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Snippet provider that loads from JSON files on disk.
///
/// Snippet bodies are stored as raw strings (lazy parsing).
/// Parsing occurs only when a snippet is expanded.
#[derive(Default)]
pub struct JsonSnippetProvider {
    /// filetype → snippet definitions
    snippets: HashMap<String, Vec<SnippetDefinition>>,
}

impl JsonSnippetProvider {
    /// Load all snippet files from a directory.
    ///
    /// Each `.json` file in the directory is loaded as a filetype.
    /// The filetype is derived from the file stem (e.g., `rust.json` → `"rust"`).
    ///
    /// If the directory does not exist, returns an empty provider.
    ///
    /// # Errors
    ///
    /// Returns `LoadError` if a file exists but cannot be read or parsed.
    pub fn load_directory(path: &Path) -> Result<Self, LoadError> {
        let mut snippets = HashMap::new();

        if !path.exists() {
            return Ok(Self { snippets });
        }

        let entries = std::fs::read_dir(path)?;
        for entry in entries {
            let entry = entry?;
            let file_path = entry.path();
            if file_path.extension().is_some_and(|ext| ext == "json")
                && let Some(stem) = file_path.file_stem().and_then(|s| s.to_str())
            {
                let filetype = stem.to_string();
                let defs = Self::load_file(&file_path)?;
                if !defs.is_empty() {
                    snippets.insert(filetype, defs);
                }
            }
        }

        Ok(Self { snippets })
    }

    /// Load snippet definitions from a single JSON file.
    ///
    /// # Errors
    ///
    /// Returns `LoadError` if the file cannot be read or parsed.
    pub fn load_file(path: &Path) -> Result<Vec<SnippetDefinition>, LoadError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse_json(&content)
    }

    /// Parse snippet definitions from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns `LoadError::Json` if the JSON is malformed.
    fn parse_json(json: &str) -> Result<Vec<SnippetDefinition>, LoadError> {
        let map: HashMap<String, RawSnippet> = serde_json::from_str(json)?;
        let mut defs = Vec::with_capacity(map.len());

        for (name, raw) in map {
            let body_raw = match raw.body {
                BodyValue::String(s) => s,
                BodyValue::Array(lines) => lines.join("\n"),
            };
            let scope = raw
                .scope
                .map(|s| {
                    s.split(',')
                        .map(|part| part.trim().to_owned())
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                })
                .filter(|v| !v.is_empty());

            // Handle prefix as string or array
            let prefixes = match raw.prefix {
                PrefixValue::Single(s) => vec![s],
                PrefixValue::Multiple(v) => v,
            };

            for prefix in prefixes {
                defs.push(SnippetDefinition {
                    name: name.clone(),
                    prefix,
                    body_raw: body_raw.clone(),
                    description: raw.description.clone(),
                    scope: scope.clone(),
                });
            }
        }

        Ok(defs)
    }
}

impl SnippetProvider for JsonSnippetProvider {
    fn snippets_for_filetype(&self, filetype: &str) -> Vec<&SnippetDefinition> {
        self.snippets
            .get(filetype)
            .map_or_else(Vec::new, |defs| defs.iter().collect())
    }

    fn snippet_by_prefix(&self, filetype: &str, prefix: &str) -> Option<&SnippetDefinition> {
        self.snippets
            .get(filetype)?
            .iter()
            .find(|def| def.prefix == prefix)
    }
}

/// Raw JSON snippet format (for serde deserialization).
#[derive(serde::Deserialize)]
struct RawSnippet {
    prefix: PrefixValue,
    body: BodyValue,
    #[serde(default)]
    description: Option<String>,
    /// VSCode-compatible language scope restriction (comma-separated).
    #[serde(default)]
    scope: Option<String>,
}

/// The `prefix` field can be a single string or array of strings.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum PrefixValue {
    Single(String),
    Multiple(Vec<String>),
}

/// The `body` field can be a string or array of strings.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum BodyValue {
    String(String),
    Array(Vec<String>),
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
