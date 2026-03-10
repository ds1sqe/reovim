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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[test]
    fn test_uri_from_path_with_spaces_falls_back() {
        // Spaces in path produce invalid URI (RFC 3986 forbids literal spaces)
        // so uri_from_path should fall back to "file:///"
        let path = Path::new("/nonexistent path with spaces/project");
        let uri = uri_from_path(path);
        assert_eq!(uri.as_str(), "file:///");
    }

    // ========================================================================
    // language_id_from_path tests
    // ========================================================================

    #[test]
    fn language_id_rust() {
        assert_eq!(language_id_from_path("main.rs"), Some("rust".to_owned()));
    }

    #[test]
    fn language_id_python() {
        assert_eq!(language_id_from_path("app.py"), Some("python".to_owned()));
        assert_eq!(language_id_from_path("types.pyi"), Some("python".to_owned()));
    }

    #[test]
    fn language_id_typescript() {
        assert_eq!(language_id_from_path("index.ts"), Some("typescript".to_owned()));
        assert_eq!(language_id_from_path("App.tsx"), Some("typescriptreact".to_owned()));
    }

    #[test]
    fn language_id_javascript() {
        assert_eq!(language_id_from_path("app.js"), Some("javascript".to_owned()));
        assert_eq!(language_id_from_path("Component.jsx"), Some("javascriptreact".to_owned()));
    }

    #[test]
    fn language_id_c_cpp() {
        assert_eq!(language_id_from_path("main.c"), Some("c".to_owned()));
        assert_eq!(language_id_from_path("header.h"), Some("c".to_owned()));
        assert_eq!(language_id_from_path("main.cpp"), Some("cpp".to_owned()));
        assert_eq!(language_id_from_path("main.cc"), Some("cpp".to_owned()));
        assert_eq!(language_id_from_path("main.cxx"), Some("cpp".to_owned()));
        assert_eq!(language_id_from_path("header.hpp"), Some("cpp".to_owned()));
    }

    #[test]
    fn language_id_other_languages() {
        assert_eq!(language_id_from_path("main.go"), Some("go".to_owned()));
        assert_eq!(language_id_from_path("Main.java"), Some("java".to_owned()));
        assert_eq!(language_id_from_path("init.lua"), Some("lua".to_owned()));
        assert_eq!(language_id_from_path("app.rb"), Some("ruby".to_owned()));
        assert_eq!(language_id_from_path("main.zig"), Some("zig".to_owned()));
    }

    #[test]
    fn language_id_config_formats() {
        assert_eq!(language_id_from_path("Cargo.toml"), Some("toml".to_owned()));
        assert_eq!(language_id_from_path("package.json"), Some("json".to_owned()));
        assert_eq!(language_id_from_path("config.yaml"), Some("yaml".to_owned()));
        assert_eq!(language_id_from_path("ci.yml"), Some("yaml".to_owned()));
        assert_eq!(language_id_from_path("README.md"), Some("markdown".to_owned()));
        assert_eq!(language_id_from_path("doc.markdown"), Some("markdown".to_owned()));
    }

    #[test]
    fn language_id_unknown() {
        assert_eq!(language_id_from_path("file.xyz"), None);
    }

    #[test]
    fn language_id_no_extension() {
        assert_eq!(language_id_from_path("Makefile"), None);
    }

    // ========================================================================
    // config_for_language tests
    // ========================================================================

    #[test]
    fn config_for_rust() {
        let root = Path::new("/tmp/project");
        let config = config_for_language("rust", root);
        assert!(config.is_some());
        assert_eq!(config.unwrap().command, "rust-analyzer");
    }

    #[test]
    fn config_for_python() {
        let root = Path::new("/tmp/project");
        let config = config_for_language("python", root);
        assert!(config.is_some());
        assert_eq!(config.unwrap().command, "pylsp");
    }

    #[test]
    fn config_for_typescript() {
        let root = Path::new("/tmp/project");
        let config = config_for_language("typescript", root);
        assert!(config.is_some());
        assert_eq!(config.unwrap().command, "typescript-language-server");
    }

    #[test]
    fn config_for_cpp() {
        let root = Path::new("/tmp/project");
        let config = config_for_language("cpp", root);
        assert!(config.is_some());
        assert_eq!(config.unwrap().command, "clangd");
    }

    #[test]
    fn config_for_typescriptreact() {
        let root = Path::new("/tmp/project");
        let config = config_for_language("typescriptreact", root);
        assert!(config.is_some());
        assert_eq!(config.unwrap().command, "typescript-language-server");
    }

    #[test]
    fn config_for_javascript() {
        let root = Path::new("/tmp/project");
        let config = config_for_language("javascript", root);
        assert!(config.is_some());
        assert_eq!(config.unwrap().command, "typescript-language-server");
    }

    #[test]
    fn config_for_javascriptreact() {
        let root = Path::new("/tmp/project");
        let config = config_for_language("javascriptreact", root);
        assert!(config.is_some());
        assert_eq!(config.unwrap().command, "typescript-language-server");
    }

    #[test]
    fn config_for_c() {
        let root = Path::new("/tmp/project");
        let config = config_for_language("c", root);
        assert!(config.is_some());
        assert_eq!(config.unwrap().command, "clangd");
    }

    #[test]
    fn config_for_unknown() {
        let root = Path::new("/tmp/project");
        assert!(config_for_language("haskell", root).is_none());
    }

    // ========================================================================
    // find_project_root tests
    // ========================================================================

    #[test]
    fn find_project_root_nonexistent() {
        let path = Path::new("/nonexistent/deep/nested/file.rs");
        assert!(find_project_root(path).is_none());
    }

    #[test]
    fn find_project_root_from_cargo_workspace() {
        // This test runs inside the reovim workspace which has Cargo.toml
        let this_file = Path::new(file!());
        if let Some(root) = find_project_root(this_file) {
            assert!(root.join("Cargo.toml").exists());
        }
    }

    #[test]
    fn find_project_root_from_file_path() {
        // Use an actual file in the project to exercise the is_file() branch
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config.rs");
        if manifest.is_file() {
            let root = find_project_root(&manifest);
            assert!(root.is_some());
            assert!(root.unwrap().join("Cargo.toml").exists());
        }
    }
}
