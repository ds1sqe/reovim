//! VFS driver traits.
//!
//! Linux equivalent: `struct file_operations`, `struct inode_operations`
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │   VfsDriver     │  <-- Main filesystem interface
//! │ (init, read,    │
//! │  write, delete) │
//! └────────┬────────┘
//!          │
//!          ├──────────────────┐
//!          │                  │
//!          ▼                  ▼
//! ┌─────────────────┐  ┌─────────────────┐
//! │   FileHandle    │  │   FileWatcher   │
//! │ (open file ops) │  │ (change events) │
//! └─────────────────┘  └─────────────────┘
//! ```
//!
//! # Usage Note
//!
//! All filesystem access goes through these VFS traits.

#![allow(clippy::missing_errors_doc)]

use {
    crate::{FileMetadata, VfsError, WatchEvent, WatchHandle},
    std::path::{Path, PathBuf},
};

/// Options for opening a file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct OpenOptions {
    /// Open for reading.
    pub read: bool,
    /// Open for writing.
    pub write: bool,
    /// Create file if it doesn't exist.
    pub create: bool,
    /// Truncate file to zero length.
    pub truncate: bool,
    /// Append to file instead of overwriting.
    pub append: bool,
}

impl OpenOptions {
    /// Create a new `OpenOptions` with all flags set to false.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            read: false,
            write: false,
            create: false,
            truncate: false,
            append: false,
        }
    }

    /// Create options for reading only.
    #[must_use]
    pub const fn read() -> Self {
        Self {
            read: true,
            write: false,
            create: false,
            truncate: false,
            append: false,
        }
    }

    /// Create options for writing (create + truncate).
    #[must_use]
    pub const fn write() -> Self {
        Self {
            read: false,
            write: true,
            create: true,
            truncate: true,
            append: false,
        }
    }

    /// Create options for appending.
    #[must_use]
    pub const fn append() -> Self {
        Self {
            read: false,
            write: true,
            create: true,
            truncate: false,
            append: true,
        }
    }

    /// Create options for read+write.
    #[must_use]
    pub const fn read_write() -> Self {
        Self {
            read: true,
            write: true,
            create: false,
            truncate: false,
            append: false,
        }
    }

    /// Create options for creating a new file (fails if exists).
    #[must_use]
    pub const fn create_new() -> Self {
        Self {
            read: false,
            write: true,
            create: true,
            truncate: false,
            append: false,
        }
    }

    /// Set the read flag.
    #[must_use]
    pub const fn with_read(mut self, read: bool) -> Self {
        self.read = read;
        self
    }

    /// Set the write flag.
    #[must_use]
    pub const fn with_write(mut self, write: bool) -> Self {
        self.write = write;
        self
    }

    /// Set the create flag.
    #[must_use]
    pub const fn with_create(mut self, create: bool) -> Self {
        self.create = create;
        self
    }

    /// Set the truncate flag.
    #[must_use]
    pub const fn with_truncate(mut self, truncate: bool) -> Self {
        self.truncate = truncate;
        self
    }

    /// Set the append flag.
    #[must_use]
    pub const fn with_append(mut self, append: bool) -> Self {
        self.append = append;
        self
    }
}

/// Seek position for file operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    /// Seek from the start of the file.
    Start(u64),
    /// Seek from the end of the file.
    End(i64),
    /// Seek from the current position.
    Current(i64),
}

impl From<std::io::SeekFrom> for SeekFrom {
    fn from(seek: std::io::SeekFrom) -> Self {
        match seek {
            std::io::SeekFrom::Start(n) => Self::Start(n),
            std::io::SeekFrom::End(n) => Self::End(n),
            std::io::SeekFrom::Current(n) => Self::Current(n),
        }
    }
}

impl From<SeekFrom> for std::io::SeekFrom {
    fn from(seek: SeekFrom) -> Self {
        match seek {
            SeekFrom::Start(n) => Self::Start(n),
            SeekFrom::End(n) => Self::End(n),
            SeekFrom::Current(n) => Self::Current(n),
        }
    }
}

/// Virtual filesystem driver.
///
/// Provides an abstraction over filesystem operations.
/// Implementations can wrap local fs, remote fs, or virtual fs.
///
/// # Replacement Mapping
///
/// This trait replaces the following `std::fs` functions:
///
/// | std::fs | VfsDriver |
/// |---------|-----------|
/// | `read()` | `read()` |
/// | `read_to_string()` | `read_to_string()` |
/// | `write()` | `write()`, `write_str()` |
/// | `remove_file()` | `delete()` |
/// | `rename()` | `rename()` |
/// | `copy()` | `copy()` |
/// | `metadata()` | `metadata()` |
/// | `symlink_metadata()` | `symlink_metadata()` |
/// | `canonicalize()` | `canonicalize()` |
/// | `read_link()` | `read_link()` |
/// | `read_dir()` | `list_dir()` |
/// | `create_dir()` | `create_dir()` |
/// | `create_dir_all()` | `create_dir_all()` |
/// | `remove_dir()` | `remove_dir()` |
/// | `remove_dir_all()` | `remove_dir_all()` |
/// | `File::open()` | `open()` |
/// | `File::create()` | `open(path, OpenOptions::write())` |
pub trait VfsDriver: Send + Sync {
    /// Initialize the VFS driver.
    fn init(&mut self) -> Result<(), VfsError>;

    /// Shutdown the VFS driver.
    fn shutdown(&mut self) -> Result<(), VfsError>;

    // ========================================================================
    // File I/O (replaces std::fs file operations)
    // ========================================================================

    /// Read entire file contents as bytes.
    ///
    /// Replaces: `std::fs::read()`
    fn read(&self, path: &Path) -> Result<Vec<u8>, VfsError>;

    /// Read entire file contents as string (UTF-8).
    ///
    /// Replaces: `std::fs::read_to_string()`
    fn read_to_string(&self, path: &Path) -> Result<String, VfsError> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes).map_err(|e| VfsError::InvalidPath(e.to_string()))
    }

    /// Write bytes to a file (creates or overwrites).
    ///
    /// Replaces: `std::fs::write()`
    fn write(&self, path: &Path, content: &[u8]) -> Result<(), VfsError>;

    /// Write string to a file (creates or overwrites).
    ///
    /// Convenience wrapper for `write()`.
    fn write_str(&self, path: &Path, content: &str) -> Result<(), VfsError> {
        self.write(path, content.as_bytes())
    }

    /// Delete a file.
    ///
    /// Replaces: `std::fs::remove_file()`
    fn delete(&self, path: &Path) -> Result<(), VfsError>;

    /// Rename/move a file or directory.
    ///
    /// Replaces: `std::fs::rename()`
    fn rename(&self, from: &Path, to: &Path) -> Result<(), VfsError>;

    /// Copy a file.
    ///
    /// Returns the number of bytes copied.
    ///
    /// Replaces: `std::fs::copy()`
    fn copy(&self, from: &Path, to: &Path) -> Result<u64, VfsError>;

    // ========================================================================
    // Query Operations
    // ========================================================================

    /// Check if a path exists.
    ///
    /// Replaces: `Path::exists()`
    fn exists(&self, path: &Path) -> bool;

    /// Get file/directory metadata.
    ///
    /// Follows symlinks.
    ///
    /// Replaces: `std::fs::metadata()`
    fn metadata(&self, path: &Path) -> Result<FileMetadata, VfsError>;

    /// Get metadata without following symlinks.
    ///
    /// Replaces: `std::fs::symlink_metadata()`
    fn symlink_metadata(&self, path: &Path) -> Result<FileMetadata, VfsError>;

    /// Canonicalize a path (resolve symlinks, make absolute).
    ///
    /// Replaces: `std::fs::canonicalize()`
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, VfsError>;

    /// Read symlink target.
    ///
    /// Replaces: `std::fs::read_link()`
    fn read_link(&self, path: &Path) -> Result<PathBuf, VfsError>;

    // ========================================================================
    // Directory Operations
    // ========================================================================

    /// List directory contents.
    ///
    /// Replaces: `std::fs::read_dir()`
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, VfsError>;

    /// Create a directory.
    ///
    /// Replaces: `std::fs::create_dir()`
    fn create_dir(&self, path: &Path) -> Result<(), VfsError>;

    /// Create a directory and all parent directories.
    ///
    /// Replaces: `std::fs::create_dir_all()`
    fn create_dir_all(&self, path: &Path) -> Result<(), VfsError>;

    /// Remove an empty directory.
    ///
    /// Replaces: `std::fs::remove_dir()`
    fn remove_dir(&self, path: &Path) -> Result<(), VfsError>;

    /// Remove a directory and all its contents.
    ///
    /// Replaces: `std::fs::remove_dir_all()`
    fn remove_dir_all(&self, path: &Path) -> Result<(), VfsError>;

    // ========================================================================
    // File Handle Operations
    // ========================================================================

    /// Open a file with specified options.
    ///
    /// Replaces: `std::fs::File::open()`, `std::fs::File::create()`
    fn open(&self, path: &Path, options: OpenOptions) -> Result<Box<dyn FileHandle>, VfsError>;

    // ========================================================================
    // Memory-Mapped I/O (large file support)
    // ========================================================================

    /// Memory-map a file for zero-copy read access.
    ///
    /// Returns a [`MappedFile`](crate::MappedFile) that implements this
    /// crate's [`FileMapping`](crate::FileMapping) trait. The default
    /// implementation falls back
    /// to reading the whole file into a heap-copied `MappedFile`.
    fn mmap_read(&self, path: &Path) -> Result<crate::MappedFile, VfsError> {
        let bytes = self.read(path)?;
        Ok(crate::MappedFile::from_vec(&bytes, path))
    }
}

/// Directory entry from `list_dir`.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Entry path (full path).
    pub path: PathBuf,
    /// Entry name (file name only).
    pub name: String,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Whether this is a file.
    pub is_file: bool,
    /// Whether this is a symlink.
    pub is_symlink: bool,
}

impl DirEntry {
    /// Create a new directory entry.
    #[must_use]
    pub fn new(path: PathBuf, is_dir: bool, is_file: bool, is_symlink: bool) -> Self {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            path,
            name,
            is_dir,
            is_file,
            is_symlink,
        }
    }

    /// Create a directory entry for a file.
    #[must_use]
    pub fn file(path: PathBuf) -> Self {
        Self::new(path, false, true, false)
    }

    /// Create a directory entry for a directory.
    #[must_use]
    pub fn directory(path: PathBuf) -> Self {
        Self::new(path, true, false, false)
    }

    /// Create a directory entry for a symlink.
    #[must_use]
    pub fn symlink(path: PathBuf) -> Self {
        Self::new(path, false, false, true)
    }
}

/// Handle to an open file.
///
/// Provides streaming read/write operations for efficient I/O
/// on large files.
///
/// Replaces: `std::fs::File`
pub trait FileHandle: Send + Sync {
    /// Read bytes into buffer, returns number of bytes read.
    ///
    /// Returns 0 when EOF is reached.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, VfsError>;

    /// Read exact number of bytes.
    ///
    /// Returns error if not enough bytes available.
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), VfsError> {
        let mut total = 0;
        while total < buf.len() {
            match self.read(&mut buf[total..])? {
                0 => {
                    return Err(VfsError::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "unexpected end of file",
                    )));
                }
                n => total += n,
            }
        }
        Ok(())
    }

    /// Read all remaining bytes into a vector.
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, VfsError> {
        let mut total = 0;
        let mut chunk = [0u8; 8192];
        loop {
            match self.read(&mut chunk)? {
                0 => break,
                n => {
                    buf.extend_from_slice(&chunk[..n]);
                    total += n;
                }
            }
        }
        Ok(total)
    }

    /// Write bytes from buffer, returns number of bytes written.
    fn write(&mut self, buf: &[u8]) -> Result<usize, VfsError>;

    /// Write all bytes from buffer.
    fn write_all(&mut self, buf: &[u8]) -> Result<(), VfsError> {
        let mut total = 0;
        while total < buf.len() {
            total += self.write(&buf[total..])?;
        }
        Ok(())
    }

    /// Seek to a position in the file.
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, VfsError>;

    /// Flush pending writes to storage.
    fn flush(&mut self) -> Result<(), VfsError>;

    /// Sync all data and metadata to storage.
    fn sync_all(&mut self) -> Result<(), VfsError> {
        self.flush()
    }

    /// Get file metadata.
    fn metadata(&self) -> Result<FileMetadata, VfsError>;

    /// Get the file path.
    fn path(&self) -> &Path;

    /// Get current position in file.
    fn position(&self) -> u64;

    /// Get the file size.
    fn size(&self) -> Result<u64, VfsError> {
        self.metadata().map(|m| m.size)
    }
}

/// File watcher for monitoring filesystem changes.
///
/// Provides a platform-independent interface for watching
/// files and directories for changes.
///
/// **Status**: Planned API. No production implementation yet.
/// Future implementations: `inotify` (Linux), `FSEvents` (macOS),
/// `ReadDirectoryChangesW` (Windows).
pub trait FileWatcher: Send + Sync {
    /// Watch a path for changes.
    ///
    /// If `recursive` is true and path is a directory,
    /// watch all subdirectories as well.
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<WatchHandle, VfsError>;

    /// Stop watching a path.
    fn unwatch(&mut self, handle: WatchHandle) -> Result<(), VfsError>;

    /// Poll for pending events (non-blocking).
    ///
    /// Returns all events that have occurred since the last poll.
    fn poll_events(&mut self) -> Vec<WatchEvent>;

    /// Check if there are pending events.
    fn has_events(&self) -> bool;

    /// Get the number of active watches.
    fn watch_count(&self) -> usize;
}

#[cfg(test)]
#[path = "traits_tests.rs"]
mod tests;
