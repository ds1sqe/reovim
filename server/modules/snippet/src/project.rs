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
#[path = "project_tests.rs"]
mod tests;
