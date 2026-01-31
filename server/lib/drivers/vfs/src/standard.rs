//! Standard VFS driver implementation.
//!
//! Provides a concrete implementation of the `VfsDriver` trait
//! that wraps `std::fs` for local filesystem operations.

use {
    crate::{DirEntry, FileHandle, FileMetadata, FilePermissions, OpenOptions, SeekFrom, VfsError},
    std::{
        fs::{self, File},
        io::{Read, Seek, Write},
        path::{Path, PathBuf},
    },
};

/// Standard VFS driver wrapping `std::fs`.
///
/// Zero-sized type (ZST) - all operations are stateless.
///
/// # Design Philosophy
///
/// - Direct wrapper around `std::fs` - no caching, no buffering
/// - Converts `std::io::Error` to `VfsError`
/// - Thread-safe (stateless)
#[derive(Debug, Clone, Copy, Default)]
pub struct StandardVfs;

impl StandardVfs {
    /// Create a new standard VFS driver.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Convert `std::fs::Metadata` to `FileMetadata`.
fn metadata_from_std(meta: &std::fs::Metadata) -> FileMetadata {
    let base = if meta.is_dir() {
        FileMetadata::directory()
    } else if meta.is_symlink() {
        FileMetadata::symlink()
    } else {
        FileMetadata::file(meta.len())
    };

    let mut result = base;

    // Add modification time if available
    if let Ok(modified) = meta.modified() {
        result = result.with_modified(modified);
    }

    // Add creation time if available
    if let Ok(created) = meta.created() {
        result = result.with_created(created);
    }

    // Add access time if available
    if let Ok(accessed) = meta.accessed() {
        result = result.with_accessed(accessed);
    }

    // Set read-only flag
    result = result.with_readonly(meta.permissions().readonly());

    // Set Unix permissions on Unix platforms
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        result = result.with_permissions(FilePermissions::from_mode(meta.permissions().mode()));
    }

    result
}

impl crate::VfsDriver for StandardVfs {
    fn init(&mut self) -> Result<(), VfsError> {
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), VfsError> {
        Ok(())
    }

    // ========================================================================
    // File I/O
    // ========================================================================

    fn read(&self, path: &Path) -> Result<Vec<u8>, VfsError> {
        fs::read(path).map_err(VfsError::Io)
    }

    fn write(&self, path: &Path, content: &[u8]) -> Result<(), VfsError> {
        fs::write(path, content).map_err(VfsError::Io)
    }

    fn delete(&self, path: &Path) -> Result<(), VfsError> {
        fs::remove_file(path).map_err(VfsError::Io)
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), VfsError> {
        fs::rename(from, to).map_err(VfsError::Io)
    }

    fn copy(&self, from: &Path, to: &Path) -> Result<u64, VfsError> {
        fs::copy(from, to).map_err(VfsError::Io)
    }

    // ========================================================================
    // Query Operations
    // ========================================================================

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn metadata(&self, path: &Path) -> Result<FileMetadata, VfsError> {
        let meta = fs::metadata(path).map_err(VfsError::Io)?;
        Ok(metadata_from_std(&meta))
    }

    fn symlink_metadata(&self, path: &Path) -> Result<FileMetadata, VfsError> {
        let meta = fs::symlink_metadata(path).map_err(VfsError::Io)?;
        Ok(metadata_from_std(&meta))
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, VfsError> {
        fs::canonicalize(path).map_err(VfsError::Io)
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, VfsError> {
        fs::read_link(path).map_err(VfsError::Io)
    }

    // ========================================================================
    // Directory Operations
    // ========================================================================

    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, VfsError> {
        let entries = fs::read_dir(path).map_err(VfsError::Io)?;
        entries
            .map(|entry| {
                let entry = entry.map_err(VfsError::Io)?;
                let path = entry.path();
                let file_type = entry.file_type().map_err(VfsError::Io)?;
                Ok(DirEntry::new(
                    path,
                    file_type.is_dir(),
                    file_type.is_file(),
                    file_type.is_symlink(),
                ))
            })
            .collect()
    }

    fn create_dir(&self, path: &Path) -> Result<(), VfsError> {
        fs::create_dir(path).map_err(VfsError::Io)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), VfsError> {
        fs::create_dir_all(path).map_err(VfsError::Io)
    }

    fn remove_dir(&self, path: &Path) -> Result<(), VfsError> {
        fs::remove_dir(path).map_err(VfsError::Io)
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), VfsError> {
        fs::remove_dir_all(path).map_err(VfsError::Io)
    }

    // ========================================================================
    // File Handle Operations
    // ========================================================================

    fn open(&self, path: &Path, options: OpenOptions) -> Result<Box<dyn FileHandle>, VfsError> {
        let mut std_opts = fs::OpenOptions::new();
        std_opts
            .read(options.read)
            .write(options.write)
            .create(options.create)
            .truncate(options.truncate)
            .append(options.append);

        let file = std_opts.open(path).map_err(VfsError::Io)?;
        Ok(Box::new(StandardFileHandle::new(file, path.to_path_buf())))
    }
}

/// Standard file handle wrapping `std::fs::File`.
pub struct StandardFileHandle {
    file: File,
    path: PathBuf,
    position: u64,
}

impl StandardFileHandle {
    /// Create a new file handle.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // File is not const-constructible
    pub fn new(file: File, path: PathBuf) -> Self {
        Self {
            file,
            path,
            position: 0,
        }
    }
}

impl FileHandle for StandardFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, VfsError> {
        let n = self.file.read(buf).map_err(VfsError::Io)?;
        self.position += n as u64;
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, VfsError> {
        let n = self.file.write(buf).map_err(VfsError::Io)?;
        self.position += n as u64;
        Ok(n)
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, VfsError> {
        let std_pos: std::io::SeekFrom = pos.into();
        self.position = self.file.seek(std_pos).map_err(VfsError::Io)?;
        Ok(self.position)
    }

    fn flush(&mut self) -> Result<(), VfsError> {
        self.file.flush().map_err(VfsError::Io)
    }

    fn sync_all(&mut self) -> Result<(), VfsError> {
        self.file.sync_all().map_err(VfsError::Io)
    }

    fn metadata(&self) -> Result<FileMetadata, VfsError> {
        let meta = self.file.metadata().map_err(VfsError::Io)?;
        Ok(metadata_from_std(&meta))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn position(&self) -> u64 {
        self.position
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::VfsDriver, std::env};

    #[test]
    fn test_standard_vfs_new() {
        let vfs = StandardVfs::new();
        assert!(!vfs.exists(Path::new("/nonexistent_path_xyz")));
    }

    #[test]
    fn test_read_write_delete() {
        let vfs = StandardVfs::new();
        let path = env::temp_dir().join("reovim_test_vfs_rwd.txt");

        // Write
        vfs.write(&path, b"hello world").unwrap();
        assert!(vfs.exists(&path));

        // Read
        let content = vfs.read(&path).unwrap();
        assert_eq!(content, b"hello world");

        // Read as string
        let content_str = vfs.read_to_string(&path).unwrap();
        assert_eq!(content_str, "hello world");

        // Delete
        vfs.delete(&path).unwrap();
        assert!(!vfs.exists(&path));
    }

    #[test]
    fn test_read_nonexistent() {
        let vfs = StandardVfs::new();
        let path = env::temp_dir().join("reovim_nonexistent_file_xyz_12345");
        let result = vfs.read(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_metadata() {
        let vfs = StandardVfs::new();
        let path = env::temp_dir().join("reovim_test_vfs_meta.txt");

        vfs.write(&path, b"test content").unwrap();
        let meta = vfs.metadata(&path).unwrap();

        assert!(meta.is_file);
        assert!(!meta.is_dir);
        assert!(!meta.is_symlink);
        assert_eq!(meta.size, 12);

        vfs.delete(&path).unwrap();
    }

    #[test]
    fn test_create_dir_and_list() {
        let vfs = StandardVfs::new();
        let dir = env::temp_dir().join("reovim_test_vfs_dir");
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");

        if vfs.exists(&dir) {
            vfs.remove_dir_all(&dir).unwrap();
        }
        vfs.create_dir(&dir).unwrap();
        assert!(vfs.exists(&dir));

        vfs.write(&file1, b"content1").unwrap();
        vfs.write(&file2, b"content2").unwrap();

        let entries = vfs.list_dir(&dir).unwrap();
        assert_eq!(entries.len(), 2);

        vfs.remove_dir_all(&dir).unwrap();
        assert!(!vfs.exists(&dir));
    }

    #[test]
    fn test_file_handle() {
        let vfs = StandardVfs::new();
        let path = env::temp_dir().join("reovim_test_vfs_handle.txt");

        {
            let mut handle = vfs.open(&path, OpenOptions::write()).unwrap();
            handle.write_all(b"hello").unwrap();
            handle.flush().unwrap();
            assert_eq!(handle.position(), 5);
        }

        {
            let mut handle = vfs.open(&path, OpenOptions::read()).unwrap();
            let mut buf = [0u8; 5];
            handle.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"hello");
            assert_eq!(handle.position(), 5);
        }

        {
            let mut handle = vfs.open(&path, OpenOptions::read()).unwrap();
            handle.seek(SeekFrom::Start(2)).unwrap();
            assert_eq!(handle.position(), 2);

            let mut buf = [0u8; 3];
            handle.read_exact(&mut buf).unwrap();
            assert_eq!(&buf, b"llo");
        }

        vfs.delete(&path).unwrap();
    }

    #[test]
    fn test_copy_and_rename() {
        let vfs = StandardVfs::new();
        let src = env::temp_dir().join("reovim_test_vfs_src.txt");
        let dst = env::temp_dir().join("reovim_test_vfs_dst.txt");
        let renamed = env::temp_dir().join("reovim_test_vfs_renamed.txt");

        vfs.write(&src, b"original").unwrap();

        let bytes = vfs.copy(&src, &dst).unwrap();
        assert_eq!(bytes, 8);
        assert!(vfs.exists(&dst));

        vfs.rename(&dst, &renamed).unwrap();
        assert!(!vfs.exists(&dst));
        assert!(vfs.exists(&renamed));

        vfs.delete(&src).unwrap();
        vfs.delete(&renamed).unwrap();
    }
}
