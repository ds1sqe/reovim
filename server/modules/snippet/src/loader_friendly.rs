//! Friendly-snippets provider (#529).
//!
//! Loads snippets from `VSCode` extension format with `package.json` manifest.
//! Supports `rafamadriz/friendly-snippets` and similar `VSCode` snippet extensions.
//!
//! # Manifest Format
//!
//! ```json
//! {
//!   "contributes": {
//!     "snippets": [
//!       { "language": "rust", "path": "./snippets/rust.json" },
//!       { "language": ["javascript", "typescript"], "path": "./snippets/js.json" }
//!     ]
//!   }
//! }
//! ```

use std::{collections::HashMap, path::Path};

use crate::{
    loader::{JsonSnippetProvider, LoadError},
    provider::{SnippetDefinition, SnippetProvider},
};

/// A snippet contribution entry from `package.json`.
#[derive(serde::Deserialize)]
struct SnippetContribution {
    language: LanguageValue,
    path: String,
}

/// The `language` field can be a single string or array of strings.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum LanguageValue {
    Single(String),
    Multiple(Vec<String>),
}

impl LanguageValue {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::Single(s) => vec![s],
            Self::Multiple(v) => v,
        }
    }
}

/// Partial `package.json` structure for snippet extensions.
#[derive(serde::Deserialize)]
struct PackageJson {
    contributes: Option<Contributes>,
}

#[derive(serde::Deserialize)]
struct Contributes {
    snippets: Option<Vec<SnippetContribution>>,
}

/// Provider that loads snippets from a friendly-snippets directory.
///
/// Reads `package.json` to discover snippet files, then loads each
/// file and maps snippets to the declared languages.
pub struct FriendlySnippetsProvider {
    /// language → snippet definitions
    snippets: HashMap<String, Vec<SnippetDefinition>>,
}

impl FriendlySnippetsProvider {
    /// Load friendly-snippets from a directory containing `package.json`.
    ///
    /// # Errors
    ///
    /// Returns `LoadError` if `package.json` cannot be read or parsed.
    /// Individual snippet files that fail to load are silently skipped.
    pub fn load(dir: &Path) -> Result<Self, LoadError> {
        let manifest_path = dir.join("package.json");
        let content = std::fs::read_to_string(&manifest_path)?;
        let pkg: PackageJson = serde_json::from_str(&content)?;

        let mut snippets: HashMap<String, Vec<SnippetDefinition>> = HashMap::new();

        let contributions = pkg.contributes.and_then(|c| c.snippets).unwrap_or_default();

        for entry in contributions {
            let snippet_path = dir.join(&entry.path);
            let Ok(defs) = JsonSnippetProvider::load_file(&snippet_path) else {
                continue; // Skip files that fail to load
            };

            let languages = entry.language.into_vec();
            for lang in &languages {
                snippets
                    .entry(lang.clone())
                    .or_default()
                    .extend(defs.iter().cloned());
            }
        }

        Ok(Self { snippets })
    }
}

impl SnippetProvider for FriendlySnippetsProvider {
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

#[cfg(test)]
#[path = "loader_friendly_tests.rs"]
mod tests;
