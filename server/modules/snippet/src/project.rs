//! Project root detection for project-local snippets (#529).
//!
//! Detects project root by walking up the directory tree looking for
//! common project markers (Cargo.toml, package.json, etc.).
//! Returns the `.reovim/snippets/` directory under the project root.

use std::path::{Path, PathBuf};

/// Project markers to identify a project root directory.
const PROJECT_MARKERS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    ".git",
];

/// Find the project root by walking up from a file path.
///
/// Checks each ancestor directory for the presence of common project
/// markers. Returns the first directory containing a marker.
#[must_use]
pub fn find_project_root(file_path: &Path) -> Option<PathBuf> {
    let mut dir = if file_path.is_file() {
        file_path.parent()?
    } else {
        file_path
    };

    loop {
        for marker in PROJECT_MARKERS {
            if dir.join(marker).exists() {
                return Some(dir.to_path_buf());
            }
        }
        dir = dir.parent()?;
    }
}

/// Get the project-local snippet directory for a given file path.
///
/// Returns `{project_root}/.reovim/snippets/` if a project root is found.
#[must_use]
pub fn project_snippet_dir(file_path: &Path) -> Option<PathBuf> {
    find_project_root(file_path).map(|root| root.join(".reovim").join("snippets"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_project_root_with_cargo_toml() {
        // This crate has a Cargo.toml, so we should find it.
        let this_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/project.rs");
        let root = find_project_root(&this_file);
        assert!(root.is_some());
        let root = root.unwrap();
        assert!(root.join("Cargo.toml").exists());
    }

    #[test]
    fn test_find_project_root_from_directory() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let root = find_project_root(&dir);
        assert!(root.is_some());
    }

    #[test]
    fn test_find_project_root_not_found() {
        // Root "/" typically has no project markers.
        let root = find_project_root(Path::new("/nonexistent/path/file.rs"));
        assert!(root.is_none());
    }

    #[test]
    fn test_project_snippet_dir_returns_reovim_snippets() {
        let this_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/project.rs");
        let dir = project_snippet_dir(&this_file);
        assert!(dir.is_some());
        let dir = dir.unwrap();
        assert!(dir.ends_with(".reovim/snippets"));
    }

    #[test]
    fn test_project_snippet_dir_not_found() {
        let dir = project_snippet_dir(Path::new("/nonexistent/path/file.rs"));
        assert!(dir.is_none());
    }
}
