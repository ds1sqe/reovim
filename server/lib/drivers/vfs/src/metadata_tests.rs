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
