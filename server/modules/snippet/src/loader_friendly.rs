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
mod tests {
    use {
        super::*,
        std::{
            io::Write,
            sync::atomic::{AtomicU32, Ordering},
        },
    };

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn make_temp_dir() -> TempDir {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("reovim-friendly-test-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn write_snippet_file(dir: &Path, name: &str, content: &str) {
        let snippets_dir = dir.join("snippets");
        std::fs::create_dir_all(&snippets_dir).unwrap();
        let mut f = std::fs::File::create(snippets_dir.join(name)).unwrap();
        write!(f, "{content}").unwrap();
    }

    fn write_manifest(dir: &Path, content: &str) {
        let mut f = std::fs::File::create(dir.join("package.json")).unwrap();
        write!(f, "{content}").unwrap();
    }

    #[test]
    fn test_load_manifest_single_language() {
        let dir = make_temp_dir();
        write_snippet_file(
            dir.path(),
            "rust.json",
            r#"{ "function": { "prefix": "fn", "body": "fn $1() {}" } }"#,
        );
        write_manifest(
            dir.path(),
            r#"{
                "contributes": {
                    "snippets": [
                        { "language": "rust", "path": "./snippets/rust.json" }
                    ]
                }
            }"#,
        );

        let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
        let snippets = provider.snippets_for_filetype("rust");
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].prefix, "fn");
    }

    #[test]
    fn test_load_manifest_multiple_languages() {
        let dir = make_temp_dir();
        write_snippet_file(
            dir.path(),
            "js.json",
            r#"{ "log": { "prefix": "log", "body": "console.log($1)" } }"#,
        );
        write_manifest(
            dir.path(),
            r#"{
                "contributes": {
                    "snippets": [
                        { "language": ["javascript", "typescript"], "path": "./snippets/js.json" }
                    ]
                }
            }"#,
        );

        let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
        assert_eq!(provider.snippets_for_filetype("javascript").len(), 1);
        assert_eq!(provider.snippets_for_filetype("typescript").len(), 1);
    }

    #[test]
    fn test_load_manifest_missing_snippet_file_skipped() {
        let dir = make_temp_dir();
        write_manifest(
            dir.path(),
            r#"{
                "contributes": {
                    "snippets": [
                        { "language": "rust", "path": "./snippets/nonexistent.json" }
                    ]
                }
            }"#,
        );

        let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
        assert!(provider.snippets_for_filetype("rust").is_empty());
    }

    #[test]
    fn test_load_manifest_missing_package_json_errors() {
        let dir = make_temp_dir();
        let result = FriendlySnippetsProvider::load(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_manifest_no_contributes() {
        let dir = make_temp_dir();
        write_manifest(dir.path(), "{}");

        let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
        assert!(provider.snippets_for_filetype("rust").is_empty());
    }

    #[test]
    fn test_load_manifest_no_snippets_key() {
        let dir = make_temp_dir();
        write_manifest(dir.path(), r#"{ "contributes": {} }"#);

        let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
        assert!(provider.snippets_for_filetype("rust").is_empty());
    }

    #[test]
    fn test_snippet_by_prefix() {
        let dir = make_temp_dir();
        write_snippet_file(
            dir.path(),
            "rust.json",
            r#"{
                "function": { "prefix": "fn", "body": "fn $1() {}" },
                "struct": { "prefix": "st", "body": "struct $1 {}" }
            }"#,
        );
        write_manifest(
            dir.path(),
            r#"{
                "contributes": {
                    "snippets": [
                        { "language": "rust", "path": "./snippets/rust.json" }
                    ]
                }
            }"#,
        );

        let provider = FriendlySnippetsProvider::load(dir.path()).unwrap();
        assert!(provider.snippet_by_prefix("rust", "fn").is_some());
        assert!(provider.snippet_by_prefix("rust", "st").is_some());
        assert!(provider.snippet_by_prefix("rust", "xyz").is_none());
        assert!(provider.snippet_by_prefix("python", "fn").is_none());
    }

    #[test]
    fn test_language_value_single() {
        let val = LanguageValue::Single("rust".to_string());
        assert_eq!(val.into_vec(), vec!["rust"]);
    }

    #[test]
    fn test_language_value_multiple() {
        let val = LanguageValue::Multiple(vec!["js".to_string(), "ts".to_string()]);
        assert_eq!(val.into_vec(), vec!["js", "ts"]);
    }
}
