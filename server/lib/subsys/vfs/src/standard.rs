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
#[cfg_attr(coverage_nightly, coverage(off))]
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

    fn mmap_read(&self, path: &Path) -> Result<crate::MappedFile, VfsError> {
        let file = File::open(path).map_err(VfsError::Io)?;
        let meta = file.metadata().map_err(VfsError::Io)?;
        let mtime = meta.modified().map_err(VfsError::Io)?;
        let size = meta.len();

        // SAFETY: File is opened read-only. SIGBUS can occur if the file is
        // truncated externally while mapped. Callers must check is_stale()
        // before accessing bytes on any write path.
        #[allow(unsafe_code)]
        let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(VfsError::Io)?;

        Ok(crate::MappedFile::new(mmap, path, mtime, size))
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
#[path = "standard_tests.rs"]
mod tests;
