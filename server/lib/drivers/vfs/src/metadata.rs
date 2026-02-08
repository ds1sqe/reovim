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
mod tests {
    use super::*;

    #[test]
    fn test_file_permissions_mode() {
        let perms = FilePermissions::from_mode(0o755);
        assert_eq!(perms.mode(), 0o755);
        assert!(perms.owner_read());
        assert!(perms.owner_write());
        assert!(perms.owner_exec());
        assert!(perms.group_read());
        assert!(!perms.group_write());
        assert!(perms.group_exec());
        assert!(perms.other_read());
        assert!(!perms.other_write());
        assert!(perms.other_exec());
    }

    #[test]
    fn test_file_permissions_defaults() {
        let file_perms = FilePermissions::default_file();
        assert_eq!(file_perms.mode(), 0o644);
        assert!(file_perms.is_readable());
        assert!(file_perms.is_writable());
        assert!(!file_perms.is_executable());

        let dir_perms = FilePermissions::default_dir();
        assert_eq!(dir_perms.mode(), 0o755);
        assert!(dir_perms.is_executable());
    }

    #[test]
    fn test_file_permissions_mode_mask() {
        // Mode should be masked to lower 9 bits
        let perms = FilePermissions::from_mode(0o7755);
        assert_eq!(perms.mode(), 0o755);
    }

    #[test]
    fn test_file_metadata_file() {
        let meta = FileMetadata::file(1024);
        assert!(meta.is_file);
        assert!(!meta.is_dir);
        assert!(!meta.is_symlink);
        assert_eq!(meta.size, 1024);
        assert!(!meta.is_readonly);
    }

    #[test]
    fn test_file_metadata_directory() {
        let meta = FileMetadata::directory();
        assert!(meta.is_dir);
        assert!(!meta.is_file);
        assert!(!meta.is_symlink);
        assert_eq!(meta.size, 0);
    }

    #[test]
    fn test_file_metadata_symlink() {
        let meta = FileMetadata::symlink();
        assert!(meta.is_symlink);
        assert!(!meta.is_file);
        assert!(!meta.is_dir);
    }

    #[test]
    fn test_file_metadata_builders() {
        let now = SystemTime::now();
        let meta = FileMetadata::file(100)
            .with_modified(now)
            .with_created(now)
            .with_accessed(now)
            .with_permissions(FilePermissions::from_mode(0o755))
            .with_readonly(true);

        assert_eq!(meta.size, 100);
        assert_eq!(meta.modified, Some(now));
        assert_eq!(meta.created, Some(now));
        assert_eq!(meta.accessed, Some(now));
        assert_eq!(meta.permissions.mode(), 0o755);
        assert!(meta.is_readonly);
    }

    #[test]
    fn test_file_metadata_default() {
        let meta = FileMetadata::default();
        assert!(meta.is_file);
        assert_eq!(meta.size, 0);
    }

    #[test]
    fn test_file_permissions_default() {
        let perms = FilePermissions::default();
        assert_eq!(perms.mode(), 0o644);
    }

    #[test]
    fn test_file_permissions_zero() {
        let perms = FilePermissions::from_mode(0o000);
        assert_eq!(perms.mode(), 0);
        assert!(!perms.owner_read());
        assert!(!perms.owner_write());
        assert!(!perms.owner_exec());
        assert!(!perms.group_read());
        assert!(!perms.group_write());
        assert!(!perms.group_exec());
        assert!(!perms.other_read());
        assert!(!perms.other_write());
        assert!(!perms.other_exec());
        assert!(!perms.is_readable());
        assert!(!perms.is_writable());
        assert!(!perms.is_executable());
    }

    #[test]
    fn test_file_permissions_all() {
        let perms = FilePermissions::from_mode(0o777);
        assert_eq!(perms.mode(), 0o777);
        assert!(perms.owner_read());
        assert!(perms.owner_write());
        assert!(perms.owner_exec());
        assert!(perms.group_read());
        assert!(perms.group_write());
        assert!(perms.group_exec());
        assert!(perms.other_read());
        assert!(perms.other_write());
        assert!(perms.other_exec());
    }

    #[test]
    fn test_file_permissions_owner_only() {
        let perms = FilePermissions::from_mode(0o700);
        assert!(perms.owner_read());
        assert!(perms.owner_write());
        assert!(perms.owner_exec());
        assert!(!perms.group_read());
        assert!(!perms.other_read());
    }

    #[test]
    fn test_file_permissions_read_only() {
        let perms = FilePermissions::from_mode(0o444);
        assert!(perms.is_readable());
        assert!(!perms.is_writable());
        assert!(!perms.is_executable());
        assert!(perms.group_read());
        assert!(perms.other_read());
    }

    #[test]
    fn test_file_permissions_equality() {
        let a = FilePermissions::from_mode(0o644);
        let b = FilePermissions::from_mode(0o644);
        let c = FilePermissions::from_mode(0o755);

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_file_permissions_clone() {
        let perms = FilePermissions::from_mode(0o755);
        let cloned = perms;
        assert_eq!(perms, cloned);
    }

    #[test]
    fn test_file_permissions_debug() {
        let perms = FilePermissions::from_mode(0o644);
        let debug = format!("{perms:?}");
        assert!(debug.contains("FilePermissions"));
    }

    #[test]
    fn test_file_metadata_file_defaults() {
        let meta = FileMetadata::file(0);
        assert!(meta.is_file);
        assert!(!meta.is_dir);
        assert!(!meta.is_symlink);
        assert!(!meta.is_readonly);
        assert_eq!(meta.size, 0);
        assert!(meta.modified.is_none());
        assert!(meta.created.is_none());
        assert!(meta.accessed.is_none());
        assert_eq!(meta.permissions.mode(), 0o644);
    }

    #[test]
    fn test_file_metadata_directory_defaults() {
        let meta = FileMetadata::directory();
        assert!(meta.is_dir);
        assert!(!meta.is_file);
        assert!(!meta.is_readonly);
        assert_eq!(meta.permissions.mode(), 0o755);
    }

    #[test]
    fn test_file_metadata_symlink_defaults() {
        let meta = FileMetadata::symlink();
        assert!(meta.is_symlink);
        assert!(!meta.is_file);
        assert!(!meta.is_dir);
        assert!(!meta.is_readonly);
        assert_eq!(meta.permissions.mode(), 0o644);
    }

    #[test]
    fn test_file_metadata_with_readonly_false() {
        let meta = FileMetadata::file(0).with_readonly(false);
        assert!(!meta.is_readonly);
    }

    #[test]
    fn test_file_metadata_chained_builders() {
        let now = SystemTime::now();
        let perms = FilePermissions::from_mode(0o600);
        let meta = FileMetadata::file(1024)
            .with_modified(now)
            .with_created(now)
            .with_accessed(now)
            .with_permissions(perms)
            .with_readonly(true);

        assert_eq!(meta.size, 1024);
        assert!(meta.is_file);
        assert!(meta.is_readonly);
        assert_eq!(meta.modified, Some(now));
        assert_eq!(meta.created, Some(now));
        assert_eq!(meta.accessed, Some(now));
        assert_eq!(meta.permissions.mode(), 0o600);
    }

    #[test]
    fn test_file_metadata_debug() {
        let meta = FileMetadata::file(42);
        let debug = format!("{meta:?}");
        assert!(debug.contains("FileMetadata"));
        assert!(debug.contains("42"));
    }

    #[test]
    fn test_file_metadata_clone() {
        let now = SystemTime::now();
        let meta = FileMetadata::file(100).with_modified(now);
        let cloned = meta;

        assert_eq!(cloned.size, 100);
        assert!(cloned.is_file);
        assert_eq!(cloned.modified, Some(now));
    }

    #[test]
    fn test_file_metadata_large_size() {
        let meta = FileMetadata::file(u64::MAX);
        assert_eq!(meta.size, u64::MAX);
    }

    #[test]
    fn test_file_permissions_individual_bits() {
        // Test each permission bit individually
        let owner_read = FilePermissions::from_mode(FilePermissions::OWNER_READ);
        assert!(owner_read.owner_read());
        assert!(!owner_read.owner_write());

        let owner_write = FilePermissions::from_mode(FilePermissions::OWNER_WRITE);
        assert!(owner_write.owner_write());
        assert!(!owner_write.owner_read());

        let owner_exec = FilePermissions::from_mode(FilePermissions::OWNER_EXEC);
        assert!(owner_exec.owner_exec());

        let group_read = FilePermissions::from_mode(FilePermissions::GROUP_READ);
        assert!(group_read.group_read());

        let group_write = FilePermissions::from_mode(FilePermissions::GROUP_WRITE);
        assert!(group_write.group_write());

        let group_exec = FilePermissions::from_mode(FilePermissions::GROUP_EXEC);
        assert!(group_exec.group_exec());

        let other_read = FilePermissions::from_mode(FilePermissions::OTHER_READ);
        assert!(other_read.other_read());

        let other_write = FilePermissions::from_mode(FilePermissions::OTHER_WRITE);
        assert!(other_write.other_write());

        let other_exec = FilePermissions::from_mode(FilePermissions::OTHER_EXEC);
        assert!(other_exec.other_exec());
    }
}
