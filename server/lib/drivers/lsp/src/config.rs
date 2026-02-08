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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_analyzer_config() {
        let root = Path::new("/tmp/test-project");
        let config = LspServerConfig::rust_analyzer(root);
        assert_eq!(config.command, "rust-analyzer");
        assert_eq!(config.root_path, root);
        assert_eq!(config.workspace_folders.len(), 1);
        assert!(config.args.is_empty());
    }

    #[test]
    fn test_clangd_config() {
        let root = Path::new("/tmp/cpp-project");
        let config = LspServerConfig::clangd(root);
        assert_eq!(config.command, "clangd");
        assert_eq!(config.root_path, root);
    }

    #[test]
    fn test_pylsp_config() {
        let root = Path::new("/tmp/python-project");
        let config = LspServerConfig::pylsp(root);
        assert_eq!(config.command, "pylsp");
        assert_eq!(config.root_path, root);
    }

    #[test]
    fn test_typescript_config() {
        let root = Path::new("/tmp/ts-project");
        let config = LspServerConfig::typescript(root);
        assert_eq!(config.command, "typescript-language-server");
        assert_eq!(config.args, vec!["--stdio"]);
    }

    #[test]
    fn test_custom_config() {
        let root = Path::new("/tmp/custom-project");
        let config = LspServerConfig::custom("my-lsp", root);
        assert_eq!(config.command, "my-lsp");
        assert_eq!(config.root_path, root);
    }

    #[test]
    fn test_with_args() {
        let root = Path::new("/tmp/test");
        let config = LspServerConfig::rust_analyzer(root).with_args(["--log-file", "/tmp/ra.log"]);
        assert_eq!(config.args, vec!["--log-file", "/tmp/ra.log"]);
    }

    #[test]
    fn test_uri_from_path_unix() {
        // Note: This test may behave differently on Windows
        let path = Path::new("/tmp/test.rs");
        let uri = uri_from_path(path);
        let uri_str = uri.as_str();
        assert!(uri_str.starts_with("file://"));
        assert!(uri_str.contains("test.rs") || uri_str.contains("tmp"));
    }

    #[test]
    fn test_workspace_folder_name() {
        let root = Path::new("/home/user/my-project");
        let config = LspServerConfig::rust_analyzer(root);
        // Name should be extracted from path
        assert!(!config.workspace_folders[0].name.is_empty());
    }

    #[test]
    fn test_config_clone() {
        let root = Path::new("/tmp/project");
        let config = LspServerConfig::rust_analyzer(root).with_args(["--log"]);
        let cloned = config.clone();
        assert_eq!(cloned.command, config.command);
        assert_eq!(cloned.args, config.args);
        assert_eq!(cloned.root_path, config.root_path);
    }

    #[test]
    fn test_config_debug() {
        let root = Path::new("/tmp/project");
        let config = LspServerConfig::rust_analyzer(root);
        let debug = format!("{config:?}");
        assert!(debug.contains("rust-analyzer"));
    }

    #[test]
    fn test_with_args_extends() {
        let root = Path::new("/tmp/project");
        let config = LspServerConfig::custom("my-lsp", root)
            .with_args(["--stdio"])
            .with_args(["--verbose"]);
        assert_eq!(config.args, vec!["--stdio", "--verbose"]);
    }

    #[test]
    fn test_typescript_has_stdio_arg() {
        let root = Path::new("/tmp/ts");
        let config = LspServerConfig::typescript(root);
        assert!(config.args.contains(&"--stdio".to_string()));
    }

    #[test]
    fn test_uri_from_path_nonexistent() {
        // Non-existent path should still produce a valid URI via fallback
        let path = Path::new("/nonexistent/path/for/uri/test");
        let uri = uri_from_path(path);
        let uri_str = uri.as_str();
        assert!(uri_str.starts_with("file://"));
    }

    #[test]
    fn test_workspace_folder_name_root_path() {
        // A root path like "/" has no file_name(), should use "workspace" fallback
        let root = Path::new("/");
        let config = LspServerConfig::rust_analyzer(root);
        assert_eq!(config.workspace_folders[0].name, "workspace");
    }

    #[test]
    fn test_uri_from_path_existing() {
        // An existing path should canonicalize and produce a file:// URI
        let path = Path::new("/tmp");
        let uri = uri_from_path(path);
        let uri_str = uri.as_str();
        assert!(uri_str.starts_with("file://"));
    }
}
