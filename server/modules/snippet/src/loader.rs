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
            defs.push(SnippetDefinition {
                name,
                prefix: raw.prefix,
                body_raw,
                description: raw.description,
            });
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
    prefix: String,
    body: BodyValue,
    #[serde(default)]
    description: Option<String>,
}

/// The `body` field can be a string or array of strings.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum BodyValue {
    String(String),
    Array(Vec<String>),
}

#[cfg(test)]
#[allow(clippy::literal_string_with_formatting_args)]
mod tests {
    use {
        super::*,
        std::{
            io::Write,
            sync::atomic::{AtomicU32, Ordering},
        },
    };

    /// Counter for unique temp directory names.
    static TEMP_COUNTER: AtomicU32 = AtomicU32::new(0);

    /// RAII guard that cleans up a temporary directory on drop.
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

    /// Create a temporary directory for testing.
    fn make_temp_dir() -> TempDir {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("reovim-snippet-test-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    // =========================================================================
    // LoadError
    // =========================================================================

    #[test]
    fn test_load_error_display_io() {
        let err =
            LoadError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));
        let display = err.to_string();
        assert!(display.contains("I/O error"));
    }

    #[test]
    fn test_load_error_display_json() {
        let json_err: Result<HashMap<String, String>, _> = serde_json::from_str("{invalid");
        let err = LoadError::Json(json_err.unwrap_err());
        let display = err.to_string();
        assert!(display.contains("JSON error"));
    }

    #[test]
    fn test_load_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let load_err: LoadError = io_err.into();
        assert!(matches!(load_err, LoadError::Io(_)));
    }

    #[test]
    fn test_load_error_from_json() {
        let json_err: Result<String, _> = serde_json::from_str("{bad");
        let load_err: LoadError = json_err.unwrap_err().into();
        assert!(matches!(load_err, LoadError::Json(_)));
    }

    #[test]
    fn test_load_error_debug() {
        let err = LoadError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        let debug = format!("{err:?}");
        assert!(debug.contains("Io"));
    }

    // =========================================================================
    // parse_json
    // =========================================================================

    #[test]
    fn test_parse_json_string_body() {
        let json = r#"{
            "function": {
                "prefix": "fn",
                "body": "fn ${1:name}() {\n\t$0\n}",
                "description": "Function definition"
            }
        }"#;
        let defs = JsonSnippetProvider::parse_json(json).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "function");
        assert_eq!(defs[0].prefix, "fn");
        assert_eq!(defs[0].body_raw, "fn ${1:name}() {\n\t$0\n}");
        assert_eq!(defs[0].description.as_deref(), Some("Function definition"));
    }

    #[test]
    fn test_parse_json_array_body() {
        let json = r#"{
            "for_loop": {
                "prefix": "for",
                "body": [
                    "for ${1:i} in ${2:iter} {",
                    "\t$0",
                    "}"
                ]
            }
        }"#;
        let defs = JsonSnippetProvider::parse_json(json).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].body_raw, "for ${1:i} in ${2:iter} {\n\t$0\n}");
    }

    #[test]
    fn test_parse_json_no_description() {
        let json = r#"{
            "test": {
                "prefix": "t",
                "body": "$1"
            }
        }"#;
        let defs = JsonSnippetProvider::parse_json(json).unwrap();
        assert!(defs[0].description.is_none());
    }

    #[test]
    fn test_parse_json_multiple_snippets() {
        let json = r#"{
            "fn": { "prefix": "fn", "body": "fn $1() {}" },
            "struct": { "prefix": "st", "body": "struct $1 {}" },
            "impl": { "prefix": "impl", "body": "impl $1 {}" }
        }"#;
        let defs = JsonSnippetProvider::parse_json(json).unwrap();
        assert_eq!(defs.len(), 3);
    }

    #[test]
    fn test_parse_json_empty() {
        let defs = JsonSnippetProvider::parse_json("{}").unwrap();
        assert!(defs.is_empty());
    }

    #[test]
    fn test_parse_json_malformed() {
        let result = JsonSnippetProvider::parse_json("{invalid json");
        assert!(result.is_err());
    }

    // =========================================================================
    // JsonSnippetProvider
    // =========================================================================

    #[test]
    fn test_default_provider_empty() {
        let provider = JsonSnippetProvider::default();
        assert!(provider.snippets_for_filetype("rust").is_empty());
        assert!(provider.snippet_by_prefix("rust", "fn").is_none());
    }

    #[test]
    fn test_load_directory_nonexistent() {
        let provider = JsonSnippetProvider::load_directory(Path::new("/nonexistent/path")).unwrap();
        assert!(provider.snippets_for_filetype("rust").is_empty());
    }

    #[test]
    fn test_load_directory_with_json_files() {
        let dir = make_temp_dir();
        let rust_path = dir.path().join("rust.json");
        let mut f = std::fs::File::create(&rust_path).unwrap();
        writeln!(f, r#"{{ "function": {{ "prefix": "fn", "body": "fn $1() {{}}" }} }}"#).unwrap();

        let provider = JsonSnippetProvider::load_directory(dir.path()).unwrap();
        let snippets = provider.snippets_for_filetype("rust");
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].prefix, "fn");
    }

    #[test]
    fn test_load_directory_ignores_non_json() {
        let dir = make_temp_dir();
        let txt_path = dir.path().join("notes.txt");
        std::fs::write(txt_path, "not json").unwrap();

        let provider = JsonSnippetProvider::load_directory(dir.path()).unwrap();
        assert!(provider.snippets_for_filetype("notes").is_empty());
    }

    #[test]
    fn test_load_file_valid() {
        let dir = make_temp_dir();
        let path = dir.path().join("test.json");
        std::fs::write(&path, r#"{ "test": { "prefix": "t", "body": "$1" } }"#).unwrap();

        let defs = JsonSnippetProvider::load_file(&path).unwrap();
        assert_eq!(defs.len(), 1);
    }

    #[test]
    fn test_load_file_missing() {
        let result = JsonSnippetProvider::load_file(Path::new("/no/such/file.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_file_malformed() {
        let dir = make_temp_dir();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "{ not valid json }").unwrap();

        let result = JsonSnippetProvider::load_file(&path);
        assert!(result.is_err());
    }

    // =========================================================================
    // SnippetProvider trait impl
    // =========================================================================

    #[test]
    fn test_snippet_by_prefix() {
        let dir = make_temp_dir();
        let path = dir.path().join("rust.json");
        std::fs::write(
            &path,
            r#"{
                "function": { "prefix": "fn", "body": "fn $1() {}" },
                "struct": { "prefix": "st", "body": "struct $1 {}" }
            }"#,
        )
        .unwrap();

        let provider = JsonSnippetProvider::load_directory(dir.path()).unwrap();
        let found = provider.snippet_by_prefix("rust", "fn").unwrap();
        assert_eq!(found.name, "function");
    }

    #[test]
    fn test_snippet_by_prefix_not_found() {
        let provider = JsonSnippetProvider::default();
        assert!(provider.snippet_by_prefix("rust", "fn").is_none());
    }

    #[test]
    fn test_snippets_for_unknown_filetype() {
        let provider = JsonSnippetProvider::default();
        assert!(provider.snippets_for_filetype("unknown").is_empty());
    }

    #[test]
    fn test_load_directory_empty_json_skipped() {
        let dir = make_temp_dir();
        // An empty JSON object {} parses to zero snippets
        let path = dir.path().join("empty.json");
        std::fs::write(&path, "{}").unwrap();

        let provider = JsonSnippetProvider::load_directory(dir.path()).unwrap();
        // The "empty" filetype should not be registered (defs.is_empty() → skip)
        assert!(provider.snippets_for_filetype("empty").is_empty());
        assert!(provider.snippets.is_empty());
    }

    #[test]
    fn test_load_directory_multiple_filetypes() {
        let dir = make_temp_dir();

        let rust_path = dir.path().join("rust.json");
        std::fs::write(
            &rust_path,
            r#"{ "function": { "prefix": "fn", "body": "fn $1() {}" } }"#,
        )
        .unwrap();

        let python_path = dir.path().join("python.json");
        std::fs::write(
            &python_path,
            r#"{ "defn": { "prefix": "def", "body": "def $1():" } }"#,
        )
        .unwrap();

        let provider = JsonSnippetProvider::load_directory(dir.path()).unwrap();
        assert_eq!(provider.snippets_for_filetype("rust").len(), 1);
        assert_eq!(provider.snippets_for_filetype("python").len(), 1);
        assert!(provider.snippet_by_prefix("rust", "fn").is_some());
        assert!(provider.snippet_by_prefix("python", "def").is_some());
    }

    #[cfg(unix)]
    #[test]
    fn test_load_directory_non_utf8_stem_skipped() {
        use std::os::unix::ffi::OsStrExt;

        let dir = make_temp_dir();

        // Create a file with non-UTF8 stem: \xff.json
        let bad_name = std::ffi::OsString::from(std::ffi::OsStr::from_bytes(b"\xff.json"));
        let bad_path = dir.path().join(bad_name);
        std::fs::write(&bad_path, r#"{ "test": { "prefix": "t", "body": "$1" } }"#).unwrap();

        let provider = JsonSnippetProvider::load_directory(dir.path()).unwrap();
        // Non-UTF8 stem cannot be converted to String → skipped
        assert!(provider.snippets.is_empty());
    }
}
