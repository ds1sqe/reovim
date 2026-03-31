//! VFS driver error types.
//!
//! Linux equivalent: `include/linux/errno.h` (filesystem errors)

use std::{fmt, path::PathBuf};

/// Errors that can occur during VFS operations.
#[derive(Debug)]
pub enum VfsError {
    /// File or directory not found.
    NotFound(PathBuf),
    /// Permission denied.
    PermissionDenied(PathBuf),
    /// File or directory already exists.
    AlreadyExists(PathBuf),
    /// Expected a directory but found a file.
    NotADirectory(PathBuf),
    /// Expected a file but found a directory.
    NotAFile(PathBuf),
    /// Cannot perform operation on a directory.
    IsADirectory(PathBuf),
    /// Directory is not empty.
    DirectoryNotEmpty(PathBuf),
    /// Invalid path format.
    InvalidPath(String),
    /// Underlying I/O error.
    Io(std::io::Error),
    /// Path is too long.
    PathTooLong(PathBuf),
    /// Read-only filesystem.
    ReadOnlyFilesystem,
    /// Operation not supported.
    NotSupported(String),
}

impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "not found: {}", path.display()),
            Self::PermissionDenied(path) => write!(f, "permission denied: {}", path.display()),
            Self::AlreadyExists(path) => write!(f, "already exists: {}", path.display()),
            Self::NotADirectory(path) => write!(f, "not a directory: {}", path.display()),
            Self::NotAFile(path) => write!(f, "not a file: {}", path.display()),
            Self::IsADirectory(path) => write!(f, "is a directory: {}", path.display()),
            Self::DirectoryNotEmpty(path) => write!(f, "directory not empty: {}", path.display()),
            Self::InvalidPath(msg) => write!(f, "invalid path: {msg}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::PathTooLong(path) => write!(f, "path too long: {}", path.display()),
            Self::ReadOnlyFilesystem => write!(f, "read-only filesystem"),
            Self::NotSupported(msg) => write!(f, "not supported: {msg}"),
        }
    }
}

impl std::error::Error for VfsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl VfsError {
    /// Create a `VfsError` from an `io::Error` with the associated file path.
    ///
    /// Maps well-known error kinds to path-carrying variants; falls back to
    /// `VfsError::Io` for everything else.
    pub fn from_io(err: std::io::Error, path: impl Into<PathBuf>) -> Self {
        use std::io::ErrorKind;
        let path = path.into();
        match err.kind() {
            ErrorKind::NotFound => Self::NotFound(path),
            ErrorKind::PermissionDenied => Self::PermissionDenied(path),
            ErrorKind::AlreadyExists => Self::AlreadyExists(path),
            _ => Self::Io(err),
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
