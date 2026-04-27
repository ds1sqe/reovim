//! LSP server configuration.

use std::path::{Path, PathBuf};

use lsp_types::{Uri, WorkspaceFolder};

/// Configuration for an LSP server.
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// Command to spawn the language server.
    pub command: String,
    /// Arguments to pass to the server.
    pub args: Vec<String>,
    /// Working directory for the server.
    pub root_path: PathBuf,
    /// Workspace folders to register.
    pub workspace_folders: Vec<WorkspaceFolder>,
}

impl LspServerConfig {
    /// Create config for rust-analyzer.
    #[must_use]
    pub fn rust_analyzer(root_path: &Path) -> Self {
        let root_uri = uri_from_path(root_path);
        Self {
            command: "rust-analyzer".to_string(),
            args: vec![],
            root_path: root_path.to_path_buf(),
            workspace_folders: vec![WorkspaceFolder {
                uri: root_uri,
                name: root_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string(),
            }],
        }
    }

    /// Create config for clangd (C/C++).
    #[must_use]
    pub fn clangd(root_path: &Path) -> Self {
        let root_uri = uri_from_path(root_path);
        Self {
            command: "clangd".to_string(),
            args: vec![],
            root_path: root_path.to_path_buf(),
            workspace_folders: vec![WorkspaceFolder {
                uri: root_uri,
                name: root_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string(),
            }],
        }
    }

    /// Create config for pylsp (Python).
    #[must_use]
    pub fn pylsp(root_path: &Path) -> Self {
        let root_uri = uri_from_path(root_path);
        Self {
            command: "pylsp".to_string(),
            args: vec![],
            root_path: root_path.to_path_buf(),
            workspace_folders: vec![WorkspaceFolder {
                uri: root_uri,
                name: root_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string(),
            }],
        }
    }

    /// Create config for TypeScript language server.
    #[must_use]
    pub fn typescript(root_path: &Path) -> Self {
        let root_uri = uri_from_path(root_path);
        Self {
            command: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            root_path: root_path.to_path_buf(),
            workspace_folders: vec![WorkspaceFolder {
                uri: root_uri,
                name: root_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string(),
            }],
        }
    }

    /// Create generic config for custom servers.
    #[must_use]
    pub fn custom(command: impl Into<String>, root_path: &Path) -> Self {
        let root_uri = uri_from_path(root_path);
        Self {
            command: command.into(),
            args: vec![],
            root_path: root_path.to_path_buf(),
            workspace_folders: vec![WorkspaceFolder {
                uri: root_uri,
                name: root_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string(),
            }],
        }
    }

    /// Add command-line arguments.
    #[must_use]
    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }
}

/// Convert a file path to a `lsp_types::Uri`.
///
/// Uses proper file:// URI encoding.
///
/// # Panics
///
/// Panics if the fallback URI cannot be parsed (should never happen).
#[must_use]
pub fn uri_from_path(path: &Path) -> Uri {
    // Canonicalize to absolute path if possible
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_str = abs_path.to_string_lossy();

    // Handle Windows paths (convert backslashes, add leading slash)
    #[cfg(windows)]
    let uri_str = format!("file:///{}", path_str.replace('\\', "/"));

    // Unix paths: file:///absolute/path (3 slashes)
    #[cfg(not(windows))]
    let uri_str = format!("file://{path_str}");

    uri_str.parse().unwrap_or_else(|_| {
        // Fallback to root
        "file:///".parse().expect("fallback URI should parse")
    })
}

/// Walk up from a file path to find the project root.
///
/// Looks for `Cargo.toml` (Rust), `package.json` (JS/TS), `pyproject.toml` (Python),
/// or `go.mod` (Go). Returns the directory containing the project marker file.
#[must_use]
pub fn find_project_root(file_path: &Path) -> Option<PathBuf> {
    let markers = ["Cargo.toml", "package.json", "pyproject.toml", "go.mod"];

    let mut dir = if file_path.is_file() {
        file_path.parent()?
    } else {
        file_path
    };

    loop {
        for marker in &markers {
            if dir.join(marker).exists() {
                return Some(dir.to_path_buf());
            }
        }
        dir = dir.parent()?;
    }
}

/// Derive the LSP language ID from a file path's extension.
///
/// Maps file extensions to LSP language identifiers used for server
/// lookup and `textDocument/didOpen` notifications.
#[must_use]
pub fn language_id_from_path(path: &str) -> Option<String> {
    let ext = Path::new(path).extension()?.to_str()?;
    let lang = match ext {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "js" => "javascript",
        "jsx" => "javascriptreact",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "go" => "go",
        "java" => "java",
        "lua" => "lua",
        "rb" => "ruby",
        "zig" => "zig",
        "toml" => "toml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        _ => return None,
    };
    Some(lang.to_owned())
}

/// Map a language ID to an `LspServerConfig` factory for known languages.
///
/// Returns `None` for languages without a configured server.
#[must_use]
pub fn config_for_language(lang: &str, root: &Path) -> Option<LspServerConfig> {
    match lang {
        "rust" => Some(LspServerConfig::rust_analyzer(root)),
        "python" => Some(LspServerConfig::pylsp(root)),
        "typescript" | "typescriptreact" | "javascript" | "javascriptreact" => {
            Some(LspServerConfig::typescript(root))
        }
        "c" | "cpp" => Some(LspServerConfig::clangd(root)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
