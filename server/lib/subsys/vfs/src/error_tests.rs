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
fn test_from_io_not_found() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
    let vfs_err = VfsError::from_io(io_err, "/test/path");
    assert!(matches!(vfs_err, VfsError::NotFound(_)));
}

#[test]
fn test_from_io_permission_denied() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
    let vfs_err = VfsError::from_io(io_err, "/test/path");
    assert!(matches!(vfs_err, VfsError::PermissionDenied(_)));
}

#[test]
fn test_from_io_already_exists() {
    let io_err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "test");
    let vfs_err = VfsError::from_io(io_err, "/test/path");
    assert!(matches!(vfs_err, VfsError::AlreadyExists(_)));
}

#[test]
fn test_from_io_other() {
    let io_err = std::io::Error::other("test");
    let vfs_err = VfsError::from_io(io_err, "/test/path");
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
fn test_from_io_interrupted() {
    let io_err = std::io::Error::new(std::io::ErrorKind::Interrupted, "interrupted");
    let vfs_err = VfsError::from_io(io_err, "/test");
    assert!(matches!(vfs_err, VfsError::Io(_)));
}

#[test]
fn test_from_io_would_block() {
    let io_err = std::io::Error::new(std::io::ErrorKind::WouldBlock, "would block");
    let vfs_err = VfsError::from_io(io_err, "/test");
    assert!(matches!(vfs_err, VfsError::Io(_)));
}

// =========================================================================
// #715 repro: From<io::Error> creates variants with empty PathBuf
// error.rs: lossy From<io::Error> replaced with from_io(err, path).
// =========================================================================

#[test]
fn b4_from_io_preserves_path() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "entity not found");
    let vfs_err = VfsError::from_io(io_err, "/some/file.txt");

    match &vfs_err {
        VfsError::NotFound(path) => {
            assert_eq!(path.to_str().unwrap(), "/some/file.txt");
            let msg = format!("{vfs_err}");
            assert!(msg.contains("/some/file.txt"), "display includes path");
        }
        other => panic!("Expected NotFound, got: {other:?}"),
    }

    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let vfs_err = VfsError::from_io(io_err, "/etc/shadow");
    match &vfs_err {
        VfsError::PermissionDenied(path) => {
            assert_eq!(path.to_str().unwrap(), "/etc/shadow");
        }
        other => panic!("Expected PermissionDenied, got: {other:?}"),
    }

    let io_err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "exists");
    let vfs_err = VfsError::from_io(io_err, "/tmp/lockfile");
    match &vfs_err {
        VfsError::AlreadyExists(path) => {
            assert_eq!(path.to_str().unwrap(), "/tmp/lockfile");
        }
        other => panic!("Expected AlreadyExists, got: {other:?}"),
    }
}

#[test]
fn b4_from_io_unknown_kind_falls_through() {
    let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
    let vfs_err = VfsError::from_io(io_err, "/irrelevant");
    assert!(matches!(vfs_err, VfsError::Io(_)));
}
