use super::*;

#[test]
fn test_open_options_new() {
    let opts = OpenOptions::new();
    assert!(!opts.read);
    assert!(!opts.write);
    assert!(!opts.create);
    assert!(!opts.truncate);
    assert!(!opts.append);
}

#[test]
fn test_open_options_read() {
    let opts = OpenOptions::read();
    assert!(opts.read);
    assert!(!opts.write);
    assert!(!opts.create);
}

#[test]
fn test_open_options_write() {
    let opts = OpenOptions::write();
    assert!(!opts.read);
    assert!(opts.write);
    assert!(opts.create);
    assert!(opts.truncate);
    assert!(!opts.append);
}

#[test]
fn test_open_options_append() {
    let opts = OpenOptions::append();
    assert!(!opts.read);
    assert!(opts.write);
    assert!(opts.create);
    assert!(!opts.truncate);
    assert!(opts.append);
}

#[test]
fn test_open_options_read_write() {
    let opts = OpenOptions::read_write();
    assert!(opts.read);
    assert!(opts.write);
    assert!(!opts.create);
    assert!(!opts.truncate);
}

#[test]
fn test_open_options_builders() {
    let opts = OpenOptions::new()
        .with_read(true)
        .with_write(true)
        .with_create(true);
    assert!(opts.read);
    assert!(opts.write);
    assert!(opts.create);
}

#[test]
fn test_seek_from_conversion() {
    let start = SeekFrom::Start(100);
    let std_start: std::io::SeekFrom = start.into();
    assert!(matches!(std_start, std::io::SeekFrom::Start(100)));

    let end = SeekFrom::End(-50);
    let std_end: std::io::SeekFrom = end.into();
    assert!(matches!(std_end, std::io::SeekFrom::End(-50)));

    let current = SeekFrom::Current(25);
    let std_current: std::io::SeekFrom = current.into();
    assert!(matches!(std_current, std::io::SeekFrom::Current(25)));

    // Reverse conversion
    let back: SeekFrom = std::io::SeekFrom::Start(200).into();
    assert!(matches!(back, SeekFrom::Start(200)));
}

#[test]
fn test_dir_entry_new() {
    let entry = DirEntry::new(PathBuf::from("/test/file.txt"), false, true, false);
    assert_eq!(entry.name, "file.txt");
    assert_eq!(entry.path, PathBuf::from("/test/file.txt"));
    assert!(entry.is_file);
    assert!(!entry.is_dir);
    assert!(!entry.is_symlink);
}

#[test]
fn test_dir_entry_file() {
    let entry = DirEntry::file(PathBuf::from("/test/doc.txt"));
    assert!(entry.is_file);
    assert!(!entry.is_dir);
}

#[test]
fn test_dir_entry_directory() {
    let entry = DirEntry::directory(PathBuf::from("/test/subdir"));
    assert!(entry.is_dir);
    assert!(!entry.is_file);
}

#[test]
fn test_dir_entry_symlink() {
    let entry = DirEntry::symlink(PathBuf::from("/test/link"));
    assert!(entry.is_symlink);
    assert!(!entry.is_file);
    assert!(!entry.is_dir);
}

#[test]
fn test_open_options_create_new() {
    let opts = OpenOptions::create_new();
    assert!(!opts.read);
    assert!(opts.write);
    assert!(opts.create);
    assert!(!opts.truncate);
    assert!(!opts.append);
}

#[test]
fn test_open_options_default() {
    let opts = OpenOptions::default();
    assert!(!opts.read);
    assert!(!opts.write);
    assert!(!opts.create);
    assert!(!opts.truncate);
    assert!(!opts.append);
}

#[test]
fn test_open_options_with_truncate() {
    let opts = OpenOptions::new().with_truncate(true);
    assert!(opts.truncate);
}

#[test]
fn test_open_options_with_append() {
    let opts = OpenOptions::new().with_append(true);
    assert!(opts.append);
}

#[test]
fn test_open_options_equality() {
    let a = OpenOptions::read();
    let b = OpenOptions::read();
    assert_eq!(a, b);

    let c = OpenOptions::write();
    assert_ne!(a, c);
}

#[test]
fn test_seek_from_reverse_conversion_end() {
    let std_end = std::io::SeekFrom::End(-100);
    let vfs_end: SeekFrom = std_end.into();
    assert!(matches!(vfs_end, SeekFrom::End(-100)));
}

#[test]
fn test_seek_from_reverse_conversion_current() {
    let std_current = std::io::SeekFrom::Current(50);
    let vfs_current: SeekFrom = std_current.into();
    assert!(matches!(vfs_current, SeekFrom::Current(50)));
}

#[test]
fn test_dir_entry_name_extraction() {
    let entry = DirEntry::file(PathBuf::from("/some/path/test.rs"));
    assert_eq!(entry.name, "test.rs");
}

#[test]
fn test_dir_entry_empty_path() {
    let entry = DirEntry::new(PathBuf::from(""), false, true, false);
    assert_eq!(entry.name, "");
}

#[test]
fn test_dir_entry_root_path() {
    let entry = DirEntry::directory(PathBuf::from("/"));
    assert_eq!(entry.name, "");
}

// Test default implementations on VfsDriver through MockVfs
#[test]
fn test_vfs_driver_read_to_string() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.add_file_str("/test.txt", "hello world");

    let content = vfs.read_to_string(Path::new("/test.txt")).unwrap();
    assert_eq!(content, "hello world");
}

#[test]
fn test_vfs_driver_read_to_string_invalid_utf8() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.add_file("/binary.bin", [0xFF, 0xFE, 0x00, 0x01]);

    let result = vfs.read_to_string(Path::new("/binary.bin"));
    assert!(matches!(result.unwrap_err(), VfsError::InvalidPath(_)));
}

#[test]
fn test_vfs_driver_write_str() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.write_str(Path::new("/test.txt"), "hello").unwrap();

    let content = vfs.read_to_string(Path::new("/test.txt")).unwrap();
    assert_eq!(content, "hello");
}

// Test FileHandle default implementations through MockFileHandle
#[test]
fn test_file_handle_read_exact() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.add_file_str("/file.txt", "abcdefgh");

    let mut handle = vfs
        .open(Path::new("/file.txt"), OpenOptions::read())
        .unwrap();
    let mut buf = [0u8; 4];
    handle.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"abcd");
}

#[test]
fn test_file_handle_read_exact_eof() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.add_file_str("/file.txt", "ab");

    let mut handle = vfs
        .open(Path::new("/file.txt"), OpenOptions::read())
        .unwrap();
    let mut buf = [0u8; 10];
    let result = handle.read_exact(&mut buf);
    assert!(result.is_err());
}

#[test]
fn test_file_handle_read_to_end() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.add_file_str("/file.txt", "all the data");

    let mut handle = vfs
        .open(Path::new("/file.txt"), OpenOptions::read())
        .unwrap();
    let mut buf = Vec::new();
    let n = handle.read_to_end(&mut buf).unwrap();
    assert_eq!(n, 12);
    assert_eq!(&buf, b"all the data");
}

#[test]
fn test_file_handle_read_to_end_empty() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.add_file_str("/empty.txt", "");

    let mut handle = vfs
        .open(Path::new("/empty.txt"), OpenOptions::read())
        .unwrap();
    let mut buf = Vec::new();
    let n = handle.read_to_end(&mut buf).unwrap();
    assert_eq!(n, 0);
    assert!(buf.is_empty());
}

#[test]
fn test_file_handle_write_all() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();

    let mut handle = vfs
        .open(Path::new("/file.txt"), OpenOptions::write())
        .unwrap();
    handle.write_all(b"complete data").unwrap();
}

#[test]
fn test_file_handle_sync_all_default() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();

    let mut handle = vfs
        .open(Path::new("/file.txt"), OpenOptions::write())
        .unwrap();
    handle.write(b"data").unwrap();
    handle.sync_all().unwrap(); // Default delegates to flush()
}

#[test]
fn test_file_handle_size() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.add_file_str("/file.txt", "12345");

    let handle = vfs
        .open(Path::new("/file.txt"), OpenOptions::read())
        .unwrap();
    let size = handle.size().unwrap();
    assert_eq!(size, 5);
}

#[test]
fn test_file_handle_size_empty() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.add_file_str("/empty.txt", "");

    let handle = vfs
        .open(Path::new("/empty.txt"), OpenOptions::read())
        .unwrap();
    let size = handle.size().unwrap();
    assert_eq!(size, 0);
}
