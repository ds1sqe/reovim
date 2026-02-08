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

impl From<std::io::Error> for VfsError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match err.kind() {
            ErrorKind::NotFound => Self::NotFound(PathBuf::new()),
            ErrorKind::PermissionDenied => Self::PermissionDenied(PathBuf::new()),
            ErrorKind::AlreadyExists => Self::AlreadyExists(PathBuf::new()),
            _ => Self::Io(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::error::Error};

    #[test]
    fn test_vfs_error_display() {
        let path = PathBuf::from("/test/path");
        assert_eq!(format!("{}", VfsError::NotFound(path.clone())), "not found: /test/path");
        assert_eq!(
            format!("{}", VfsError::PermissionDenied(path.clone())),
            "permission denied: /test/path"
        );
        assert_eq!(
            format!("{}", VfsError::AlreadyExists(path.clone())),
            "already exists: /test/path"
        );
        assert_eq!(
            format!("{}", VfsError::NotADirectory(path.clone())),
            "not a directory: /test/path"
        );
        assert_eq!(format!("{}", VfsError::NotAFile(path.clone())), "not a file: /test/path");
        assert_eq!(
            format!("{}", VfsError::IsADirectory(path.clone())),
            "is a directory: /test/path"
        );
        assert_eq!(
            format!("{}", VfsError::DirectoryNotEmpty(path.clone())),
            "directory not empty: /test/path"
        );
        assert_eq!(format!("{}", VfsError::InvalidPath("bad".into())), "invalid path: bad");
        assert_eq!(format!("{}", VfsError::PathTooLong(path)), "path too long: /test/path");
        assert_eq!(format!("{}", VfsError::ReadOnlyFilesystem), "read-only filesystem");
        assert_eq!(format!("{}", VfsError::NotSupported("op".into())), "not supported: op");
    }

    #[test]
    fn test_from_io_error_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let vfs_err: VfsError = io_err.into();
        assert!(matches!(vfs_err, VfsError::NotFound(_)));
    }

    #[test]
    fn test_from_io_error_permission_denied() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
        let vfs_err: VfsError = io_err.into();
        assert!(matches!(vfs_err, VfsError::PermissionDenied(_)));
    }

    #[test]
    fn test_from_io_error_already_exists() {
        let io_err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "test");
        let vfs_err: VfsError = io_err.into();
        assert!(matches!(vfs_err, VfsError::AlreadyExists(_)));
    }

    #[test]
    fn test_from_io_error_other() {
        let io_err = std::io::Error::other("test");
        let vfs_err: VfsError = io_err.into();
        assert!(matches!(vfs_err, VfsError::Io(_)));
    }

    #[test]
    fn test_error_source() {
        let io_err = std::io::Error::other("test");
        let vfs_err = VfsError::Io(io_err);
        assert!(vfs_err.source().is_some());

        let vfs_err = VfsError::NotFound(PathBuf::new());
        assert!(vfs_err.source().is_none());
    }

    #[test]
    fn test_vfs_error_io_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
        let vfs_err = VfsError::Io(io_err);
        let display = format!("{vfs_err}");
        assert!(display.starts_with("I/O error: "));
        assert!(display.contains("pipe broke"));
    }

    #[test]
    fn test_vfs_error_debug() {
        let err = VfsError::NotFound(PathBuf::from("/test"));
        let debug = format!("{err:?}");
        assert!(debug.contains("NotFound"));
    }

    #[test]
    fn test_error_source_all_variants() {
        // All non-Io variants should return None for source()
        let variants: Vec<VfsError> = vec![
            VfsError::PermissionDenied(PathBuf::new()),
            VfsError::AlreadyExists(PathBuf::new()),
            VfsError::NotADirectory(PathBuf::new()),
            VfsError::NotAFile(PathBuf::new()),
            VfsError::IsADirectory(PathBuf::new()),
            VfsError::DirectoryNotEmpty(PathBuf::new()),
            VfsError::InvalidPath("test".into()),
            VfsError::PathTooLong(PathBuf::new()),
            VfsError::ReadOnlyFilesystem,
            VfsError::NotSupported("test".into()),
        ];

        for err in variants {
            assert!(err.source().is_none(), "Expected None source for {err}");
        }
    }

    #[test]
    fn test_from_io_error_interrupted() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Interrupted, "interrupted");
        let vfs_err: VfsError = io_err.into();
        assert!(matches!(vfs_err, VfsError::Io(_)));
    }

    #[test]
    fn test_from_io_error_would_block() {
        let io_err = std::io::Error::new(std::io::ErrorKind::WouldBlock, "would block");
        let vfs_err: VfsError = io_err.into();
        assert!(matches!(vfs_err, VfsError::Io(_)));
    }
}
