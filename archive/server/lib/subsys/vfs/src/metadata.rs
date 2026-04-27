//! File metadata types.
//!
//! Linux equivalent: `struct stat`, `struct inode`

use std::time::SystemTime;

/// File metadata information.
///
/// Mirrors `std::fs::Metadata` but is VFS-agnostic.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct FileMetadata {
    /// File size in bytes.
    pub size: u64,
    /// Last modification time.
    pub modified: Option<SystemTime>,
    /// Creation time (platform-dependent).
    pub created: Option<SystemTime>,
    /// Last access time.
    pub accessed: Option<SystemTime>,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Whether this is a regular file.
    pub is_file: bool,
    /// Whether this is a symbolic link.
    pub is_symlink: bool,
    /// Whether the file is read-only.
    pub is_readonly: bool,
    /// File permissions.
    pub permissions: FilePermissions,
}

impl FileMetadata {
    /// Create metadata for a regular file.
    #[must_use]
    pub const fn file(size: u64) -> Self {
        Self {
            size,
            modified: None,
            created: None,
            accessed: None,
            is_dir: false,
            is_file: true,
            is_symlink: false,
            is_readonly: false,
            permissions: FilePermissions::default_file(),
        }
    }

    /// Create metadata for a directory.
    #[must_use]
    pub const fn directory() -> Self {
        Self {
            size: 0,
            modified: None,
            created: None,
            accessed: None,
            is_dir: true,
            is_file: false,
            is_symlink: false,
            is_readonly: false,
            permissions: FilePermissions::default_dir(),
        }
    }

    /// Create metadata for a symbolic link.
    #[must_use]
    pub const fn symlink() -> Self {
        Self {
            size: 0,
            modified: None,
            created: None,
            accessed: None,
            is_dir: false,
            is_file: false,
            is_symlink: true,
            is_readonly: false,
            permissions: FilePermissions::default_file(),
        }
    }

    /// Set the modification time.
    #[must_use]
    pub const fn with_modified(mut self, time: SystemTime) -> Self {
        self.modified = Some(time);
        self
    }

    /// Set the creation time.
    #[must_use]
    pub const fn with_created(mut self, time: SystemTime) -> Self {
        self.created = Some(time);
        self
    }

    /// Set the access time.
    #[must_use]
    pub const fn with_accessed(mut self, time: SystemTime) -> Self {
        self.accessed = Some(time);
        self
    }

    /// Set the permissions.
    #[must_use]
    pub const fn with_permissions(mut self, permissions: FilePermissions) -> Self {
        self.permissions = permissions;
        self
    }

    /// Set the read-only flag.
    #[must_use]
    pub const fn with_readonly(mut self, readonly: bool) -> Self {
        self.is_readonly = readonly;
        self
    }
}

impl Default for FileMetadata {
    fn default() -> Self {
        Self::file(0)
    }
}

/// Unix-style file permissions.
///
/// Uses standard Unix mode bits (e.g., 0o755, 0o644).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilePermissions {
    /// Unix mode bits (e.g., 0o755).
    mode: u32,
}

impl FilePermissions {
    /// Permission bits for owner read.
    pub const OWNER_READ: u32 = 0o400;
    /// Permission bits for owner write.
    pub const OWNER_WRITE: u32 = 0o200;
    /// Permission bits for owner execute.
    pub const OWNER_EXEC: u32 = 0o100;
    /// Permission bits for group read.
    pub const GROUP_READ: u32 = 0o040;
    /// Permission bits for group write.
    pub const GROUP_WRITE: u32 = 0o020;
    /// Permission bits for group execute.
    pub const GROUP_EXEC: u32 = 0o010;
    /// Permission bits for other read.
    pub const OTHER_READ: u32 = 0o004;
    /// Permission bits for other write.
    pub const OTHER_WRITE: u32 = 0o002;
    /// Permission bits for other execute.
    pub const OTHER_EXEC: u32 = 0o001;

    /// Create permissions from raw mode bits.
    #[must_use]
    pub const fn from_mode(mode: u32) -> Self {
        Self { mode: mode & 0o777 }
    }

    /// Get the raw mode bits.
    #[must_use]
    pub const fn mode(&self) -> u32 {
        self.mode
    }

    /// Default permissions for a new file (0o644).
    #[must_use]
    pub const fn default_file() -> Self {
        Self::from_mode(0o644)
    }

    /// Default permissions for a new directory (0o755).
    #[must_use]
    pub const fn default_dir() -> Self {
        Self::from_mode(0o755)
    }

    /// Check if owner can read.
    #[must_use]
    pub const fn owner_read(&self) -> bool {
        self.mode & Self::OWNER_READ != 0
    }

    /// Check if owner can write.
    #[must_use]
    pub const fn owner_write(&self) -> bool {
        self.mode & Self::OWNER_WRITE != 0
    }

    /// Check if owner can execute.
    #[must_use]
    pub const fn owner_exec(&self) -> bool {
        self.mode & Self::OWNER_EXEC != 0
    }

    /// Check if group can read.
    #[must_use]
    pub const fn group_read(&self) -> bool {
        self.mode & Self::GROUP_READ != 0
    }

    /// Check if group can write.
    #[must_use]
    pub const fn group_write(&self) -> bool {
        self.mode & Self::GROUP_WRITE != 0
    }

    /// Check if group can execute.
    #[must_use]
    pub const fn group_exec(&self) -> bool {
        self.mode & Self::GROUP_EXEC != 0
    }

    /// Check if others can read.
    #[must_use]
    pub const fn other_read(&self) -> bool {
        self.mode & Self::OTHER_READ != 0
    }

    /// Check if others can write.
    #[must_use]
    pub const fn other_write(&self) -> bool {
        self.mode & Self::OTHER_WRITE != 0
    }

    /// Check if others can execute.
    #[must_use]
    pub const fn other_exec(&self) -> bool {
        self.mode & Self::OTHER_EXEC != 0
    }

    /// Check if file is readable (owner).
    #[must_use]
    pub const fn is_readable(&self) -> bool {
        self.owner_read()
    }

    /// Check if file is writable (owner).
    #[must_use]
    pub const fn is_writable(&self) -> bool {
        self.owner_write()
    }

    /// Check if file is executable (owner).
    #[must_use]
    pub const fn is_executable(&self) -> bool {
        self.owner_exec()
    }
}

impl Default for FilePermissions {
    fn default() -> Self {
        Self::default_file()
    }
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
