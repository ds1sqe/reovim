//! Mock VFS driver for testing.
//!
//! Provides an in-memory VFS implementation that:
//! - Tracks method calls (reads, writes)
//! - Allows pre-populating files
//! - Supports error injection for testing error paths

use {
    crate::{DirEntry, FileHandle, FileMetadata, OpenOptions, SeekFrom, VfsError},
    std::{
        collections::HashMap,
        io::Cursor,
        path::{Path, PathBuf},
        sync::Mutex,
    },
};

/// Error kind for injection (Clone-able, unlike `VfsError`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockErrorKind {
    /// File not found.
    NotFound,
    /// Permission denied.
    PermissionDenied,
    /// Is a directory.
    IsADirectory,
    /// Not a directory.
    NotADirectory,
    /// Already exists.
    AlreadyExists,
}

impl MockErrorKind {
    /// Convert to `VfsError` with the given path.
    fn to_vfs_error(self, path: &Path) -> VfsError {
        match self {
            Self::NotFound => VfsError::NotFound(path.to_path_buf()),
            Self::PermissionDenied => VfsError::PermissionDenied(path.to_path_buf()),
            Self::IsADirectory => VfsError::IsADirectory(path.to_path_buf()),
            Self::NotADirectory => VfsError::NotADirectory(path.to_path_buf()),
            Self::AlreadyExists => VfsError::AlreadyExists(path.to_path_buf()),
        }
    }
}

/// Mock VFS for testing - tracks operations and allows injecting responses.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_vfs::{MockVfs, VfsDriver};
/// use std::path::Path;
///
/// let vfs = MockVfs::new();
/// vfs.add_file("/test.txt", "hello world");
///
/// let content = vfs.read_to_string(Path::new("/test.txt")).unwrap();
/// assert_eq!(content, "hello world");
/// assert_eq!(vfs.read_calls().len(), 1);
/// ```
#[allow(clippy::zero_sized_map_values)]
pub struct MockVfs {
    /// In-memory file storage.
    files: Mutex<HashMap<PathBuf, Vec<u8>>>,
    /// In-memory directory storage (using `HashMap` with unit value for simplicity).
    directories: Mutex<HashMap<PathBuf, ()>>,
    /// Configured errors per path.
    errors: Mutex<HashMap<PathBuf, MockErrorKind>>,
    /// Recorded read operations.
    read_calls: Mutex<Vec<PathBuf>>,
    /// Recorded write operations (path, content).
    write_calls: Mutex<Vec<(PathBuf, Vec<u8>)>>,
    /// Recorded delete operations.
    delete_calls: Mutex<Vec<PathBuf>>,
}

impl MockVfs {
    /// Create a new empty mock VFS.
    #[must_use]
    #[allow(clippy::zero_sized_map_values)]
    pub fn new() -> Self {
        Self {
            files: Mutex::new(HashMap::new()),
            directories: Mutex::new(HashMap::new()),
            errors: Mutex::new(HashMap::new()),
            read_calls: Mutex::new(Vec::new()),
            write_calls: Mutex::new(Vec::new()),
            delete_calls: Mutex::new(Vec::new()),
        }
    }

    /// Pre-populate a file for reading.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn add_file(&self, path: impl AsRef<Path>, content: impl AsRef<[u8]>) {
        self.files
            .lock()
            .unwrap()
            .insert(path.as_ref().to_path_buf(), content.as_ref().to_vec());
    }

    /// Pre-populate a file with string content.
    pub fn add_file_str(&self, path: impl AsRef<Path>, content: &str) {
        self.add_file(path, content.as_bytes());
    }

    /// Add a directory.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn add_dir(&self, path: impl AsRef<Path>) {
        self.directories
            .lock()
            .unwrap()
            .insert(path.as_ref().to_path_buf(), ());
    }

    /// Configure an error for a specific path.
    ///
    /// When any operation is performed on this path, the configured error will be returned.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn set_error(&self, path: impl AsRef<Path>, error: MockErrorKind) {
        self.errors
            .lock()
            .unwrap()
            .insert(path.as_ref().to_path_buf(), error);
    }

    /// Clear a configured error for a path.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn clear_error(&self, path: impl AsRef<Path>) {
        self.errors.lock().unwrap().remove(path.as_ref());
    }

    /// Get all paths that were read.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn read_calls(&self) -> Vec<PathBuf> {
        self.read_calls.lock().unwrap().clone()
    }

    /// Get all (path, content) pairs that were written.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn write_calls(&self) -> Vec<(PathBuf, Vec<u8>)> {
        self.write_calls.lock().unwrap().clone()
    }

    /// Get all paths that were deleted.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn delete_calls(&self) -> Vec<PathBuf> {
        self.delete_calls.lock().unwrap().clone()
    }

    /// Clear all recorded calls.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn clear_calls(&self) {
        self.read_calls.lock().unwrap().clear();
        self.write_calls.lock().unwrap().clear();
        self.delete_calls.lock().unwrap().clear();
    }

    /// Check if a configured error exists for a path.
    fn check_error(&self, path: &Path) -> Option<VfsError> {
        self.errors
            .lock()
            .unwrap()
            .get(path)
            .map(|kind| kind.to_vfs_error(path))
    }
}

impl Default for MockVfs {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::VfsDriver for MockVfs {
    fn init(&mut self) -> Result<(), VfsError> {
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), VfsError> {
        Ok(())
    }

    fn read(&self, path: &Path) -> Result<Vec<u8>, VfsError> {
        self.read_calls.lock().unwrap().push(path.to_path_buf());

        if let Some(err) = self.check_error(path) {
            return Err(err);
        }

        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))
    }

    fn write(&self, path: &Path, content: &[u8]) -> Result<(), VfsError> {
        self.write_calls
            .lock()
            .unwrap()
            .push((path.to_path_buf(), content.to_vec()));

        if let Some(err) = self.check_error(path) {
            return Err(err);
        }

        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), content.to_vec());
        Ok(())
    }

    fn delete(&self, path: &Path) -> Result<(), VfsError> {
        self.delete_calls.lock().unwrap().push(path.to_path_buf());

        if let Some(err) = self.check_error(path) {
            return Err(err);
        }

        self.files
            .lock()
            .unwrap()
            .remove(path)
            .map(drop)
            .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), VfsError> {
        if let Some(err) = self.check_error(from) {
            return Err(err);
        }

        let content = self
            .files
            .lock()
            .unwrap()
            .remove(from)
            .ok_or_else(|| VfsError::NotFound(from.to_path_buf()))?;

        self.files.lock().unwrap().insert(to.to_path_buf(), content);
        Ok(())
    }

    fn copy(&self, from: &Path, to: &Path) -> Result<u64, VfsError> {
        if let Some(err) = self.check_error(from) {
            return Err(err);
        }

        let content = self
            .files
            .lock()
            .unwrap()
            .get(from)
            .cloned()
            .ok_or_else(|| VfsError::NotFound(from.to_path_buf()))?;

        let len = content.len() as u64;
        self.files.lock().unwrap().insert(to.to_path_buf(), content);
        Ok(len)
    }

    fn exists(&self, path: &Path) -> bool {
        self.files.lock().unwrap().contains_key(path)
            || self.directories.lock().unwrap().contains_key(path)
    }

    fn metadata(&self, path: &Path) -> Result<FileMetadata, VfsError> {
        if let Some(err) = self.check_error(path) {
            return Err(err);
        }

        if let Some(content) = self.files.lock().unwrap().get(path) {
            return Ok(FileMetadata::file(content.len() as u64));
        }

        if self.directories.lock().unwrap().contains_key(path) {
            return Ok(FileMetadata::directory());
        }

        Err(VfsError::NotFound(path.to_path_buf()))
    }

    fn symlink_metadata(&self, path: &Path) -> Result<FileMetadata, VfsError> {
        self.metadata(path)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, VfsError> {
        if self.exists(path) {
            Ok(path.to_path_buf())
        } else {
            Err(VfsError::NotFound(path.to_path_buf()))
        }
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, VfsError> {
        Err(VfsError::NotSupported(format!(
            "read_link not supported in MockVfs: {}",
            path.display()
        )))
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, VfsError> {
        if let Some(err) = self.check_error(path) {
            return Err(err);
        }

        if !self.directories.lock().unwrap().contains_key(path) {
            return Err(VfsError::NotADirectory(path.to_path_buf()));
        }

        // Collect paths while holding locks briefly to avoid significant_drop_tightening
        let file_paths: Vec<_> = self.files.lock().unwrap().keys().cloned().collect();
        let dir_paths: Vec<_> = self.directories.lock().unwrap().keys().cloned().collect();

        let mut entries = Vec::new();

        for file_path in file_paths {
            if let Some(parent) = file_path.parent()
                && parent == path
            {
                entries.push(DirEntry::file(file_path));
            }
        }

        for dir_path in dir_paths {
            if let Some(parent) = dir_path.parent()
                && parent == path
                && dir_path != path
            {
                entries.push(DirEntry::directory(dir_path));
            }
        }

        Ok(entries)
    }

    fn create_dir(&self, path: &Path) -> Result<(), VfsError> {
        if let Some(err) = self.check_error(path) {
            return Err(err);
        }

        if self.directories.lock().unwrap().contains_key(path) {
            return Err(VfsError::AlreadyExists(path.to_path_buf()));
        }

        self.directories
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), ());
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), VfsError> {
        let mut current = PathBuf::new();
        for component in path.components() {
            current.push(component);
            self.directories
                .lock()
                .unwrap()
                .entry(current.clone())
                .or_insert(());
        }
        Ok(())
    }

    fn remove_dir(&self, path: &Path) -> Result<(), VfsError> {
        if let Some(err) = self.check_error(path) {
            return Err(err);
        }

        self.directories
            .lock()
            .unwrap()
            .remove(path)
            .map(drop)
            .ok_or_else(|| VfsError::NotFound(path.to_path_buf()))
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), VfsError> {
        if let Some(err) = self.check_error(path) {
            return Err(err);
        }

        let path_str = path.to_string_lossy();
        self.files
            .lock()
            .unwrap()
            .retain(|k, _v| !k.to_string_lossy().starts_with(path_str.as_ref()));
        self.directories
            .lock()
            .unwrap()
            .retain(|k, ()| !k.to_string_lossy().starts_with(path_str.as_ref()));
        Ok(())
    }

    fn open(&self, path: &Path, options: OpenOptions) -> Result<Box<dyn FileHandle>, VfsError> {
        if let Some(err) = self.check_error(path) {
            return Err(err);
        }

        let content = if options.read {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Box::new(MockFileHandle::new(path.to_path_buf(), content, options.write)))
    }
}

/// Mock file handle for in-memory operations.
pub struct MockFileHandle {
    path: PathBuf,
    cursor: Cursor<Vec<u8>>,
    #[allow(dead_code)]
    writable: bool,
}

impl MockFileHandle {
    /// Create a new mock file handle.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Cursor::new is not const
    pub fn new(path: PathBuf, content: Vec<u8>, writable: bool) -> Self {
        Self {
            path,
            cursor: Cursor::new(content),
            writable,
        }
    }
}

impl FileHandle for MockFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, VfsError> {
        use std::io::Read;
        self.cursor.read(buf).map_err(VfsError::Io)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, VfsError> {
        use std::io::Write;
        self.cursor.write(buf).map_err(VfsError::Io)
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, VfsError> {
        use std::io::Seek;
        self.cursor.seek(pos.into()).map_err(VfsError::Io)
    }

    fn flush(&mut self) -> Result<(), VfsError> {
        Ok(())
    }

    fn metadata(&self) -> Result<FileMetadata, VfsError> {
        Ok(FileMetadata::file(self.cursor.get_ref().len() as u64))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn position(&self) -> u64 {
        self.cursor.position()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::VfsDriver};

    #[test]
    fn test_mock_vfs_new() {
        let vfs = MockVfs::new();
        assert!(!vfs.exists(Path::new("/test")));
    }

    #[test]
    fn test_mock_vfs_add_file() {
        let vfs = MockVfs::new();
        vfs.add_file_str("/test.txt", "hello world");

        assert!(vfs.exists(Path::new("/test.txt")));
        let content = vfs.read_to_string(Path::new("/test.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_mock_vfs_write_read() {
        let vfs = MockVfs::new();

        vfs.write(Path::new("/new.txt"), b"new content").unwrap();
        assert!(vfs.exists(Path::new("/new.txt")));

        let content = vfs.read(Path::new("/new.txt")).unwrap();
        assert_eq!(content, b"new content");
    }

    #[test]
    fn test_mock_vfs_read_calls() {
        let vfs = MockVfs::new();
        vfs.add_file_str("/file1.txt", "content1");
        vfs.add_file_str("/file2.txt", "content2");

        let _ = vfs.read(Path::new("/file1.txt"));
        let _ = vfs.read(Path::new("/file2.txt"));
        let _ = vfs.read(Path::new("/file1.txt"));

        let calls = vfs.read_calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], PathBuf::from("/file1.txt"));
        assert_eq!(calls[1], PathBuf::from("/file2.txt"));
        assert_eq!(calls[2], PathBuf::from("/file1.txt"));
    }

    #[test]
    fn test_mock_vfs_write_calls() {
        let vfs = MockVfs::new();

        vfs.write(Path::new("/a.txt"), b"aaa").unwrap();
        vfs.write(Path::new("/b.txt"), b"bbb").unwrap();

        let calls = vfs.write_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], (PathBuf::from("/a.txt"), b"aaa".to_vec()));
        assert_eq!(calls[1], (PathBuf::from("/b.txt"), b"bbb".to_vec()));
    }

    #[test]
    fn test_mock_vfs_error_injection() {
        let vfs = MockVfs::new();
        vfs.add_file_str("/test.txt", "content");
        vfs.set_error("/test.txt", MockErrorKind::PermissionDenied);

        let result = vfs.read(Path::new("/test.txt"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VfsError::PermissionDenied(_)));
    }

    #[test]
    fn test_mock_vfs_not_found() {
        let vfs = MockVfs::new();

        let result = vfs.read(Path::new("/nonexistent"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VfsError::NotFound(_)));
    }

    #[test]
    fn test_mock_vfs_delete() {
        let vfs = MockVfs::new();
        vfs.add_file_str("/file.txt", "content");

        assert!(vfs.exists(Path::new("/file.txt")));
        vfs.delete(Path::new("/file.txt")).unwrap();
        assert!(!vfs.exists(Path::new("/file.txt")));

        let calls = vfs.delete_calls();
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn test_mock_vfs_metadata() {
        let vfs = MockVfs::new();
        vfs.add_file_str("/file.txt", "12345");

        let meta = vfs.metadata(Path::new("/file.txt")).unwrap();
        assert!(meta.is_file);
        assert_eq!(meta.size, 5);
    }

    #[test]
    fn test_mock_vfs_directory() {
        let vfs = MockVfs::new();
        vfs.add_dir("/mydir");
        vfs.add_file_str("/mydir/file.txt", "content");

        assert!(vfs.exists(Path::new("/mydir")));

        let meta = vfs.metadata(Path::new("/mydir")).unwrap();
        assert!(meta.is_dir);

        let entries = vfs.list_dir(Path::new("/mydir")).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_file);
    }

    #[test]
    fn test_mock_vfs_clear_calls() {
        let vfs = MockVfs::new();
        vfs.add_file_str("/test.txt", "content");

        let _ = vfs.read(Path::new("/test.txt"));
        assert_eq!(vfs.read_calls().len(), 1);

        vfs.clear_calls();
        assert!(vfs.read_calls().is_empty());
    }
}
