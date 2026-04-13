//! Tests for `MappedFile`.

use std::sync::Arc;

use crate::{FileMapping, MappedFile};

#[test]
fn from_vec_basic() {
    let path = std::path::Path::new("/test/file.txt");
    let mf = MappedFile::from_vec(b"hello world", path);
    assert_eq!(mf.as_bytes(), b"hello world");
    assert_eq!(mf.len(), 11);
    assert_eq!(mf.path(), path);
    assert_eq!(mf.size(), 11);
    assert!(!mf.is_empty());
}

#[test]
fn from_vec_empty() {
    let path = std::path::Path::new("/test/empty");
    let mf = MappedFile::from_vec(b"", path);
    assert!(mf.is_empty());
    assert_eq!(mf.len(), 0);
    assert_eq!(mf.as_bytes(), b"");
}

#[test]
fn clone_shares_inner() {
    let path = std::path::Path::new("/test/shared");
    let mf1 = MappedFile::from_vec(b"shared data", path);
    let mf2 = mf1.clone();

    assert_eq!(mf1.as_bytes(), mf2.as_bytes());
    assert_eq!(mf1.size(), mf2.size());
}

#[test]
fn arc_file_mapping() {
    let path = std::path::Path::new("/test/arc");
    let mf = MappedFile::from_vec(b"arc test", path);
    let fm: Arc<dyn FileMapping> = Arc::new(mf);

    assert_eq!(fm.as_bytes(), b"arc test");
    assert_eq!(fm.len(), 8);
    // is_stale returns true for from_vec with fake paths (file doesn't exist)
    assert!(fm.is_stale());
}

#[test]
fn is_stale_nonexistent_path() {
    let path = std::path::Path::new("/nonexistent/path/that/doesnt/exist.txt");
    let mf = MappedFile::from_vec(b"test", path);
    // Non-existent file returns stale=true (real mmap would too)
    assert!(mf.is_stale());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn debug_formatting() {
    let path = std::path::Path::new("/test/debug");
    let mf = MappedFile::from_vec(b"test", path);
    let debug = format!("{mf:?}");
    assert!(debug.contains("MappedFile"));
    assert!(debug.contains("size"));
}

#[test]
fn real_mmap_read() {
    use {
        crate::{StandardVfs, VfsDriver},
        std::env,
    };

    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_mmap_test.txt");

    // Write a test file
    vfs.write(&path, b"mmap test content").unwrap();

    // mmap_read it
    let mf = vfs.mmap_read(&path).unwrap();
    assert_eq!(mf.as_bytes(), b"mmap test content");
    assert_eq!(mf.size(), 17);
    assert!(!mf.is_stale());

    // Cleanup
    vfs.delete(&path).unwrap();
}

#[test]
fn mmap_read_nonexistent() {
    use crate::{StandardVfs, VfsDriver};

    let vfs = StandardVfs::new();
    let result = vfs.mmap_read(std::path::Path::new("/nonexistent/file.txt"));
    assert!(result.is_err());
}

#[test]
fn default_mmap_read_fallback() {
    use crate::{MockVfs, VfsDriver};

    let vfs = MockVfs::new();
    vfs.add_file_str("/test.txt", "fallback content");

    let mf = vfs.mmap_read(std::path::Path::new("/test.txt")).unwrap();
    assert_eq!(mf.as_bytes(), b"fallback content");
}

#[test]
fn is_stale_mtime_changed_only() {
    use {
        crate::{StandardVfs, VfsDriver},
        std::env,
    };

    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_stale_mtime_only.txt");
    vfs.write(&path, b"hello").unwrap();

    let mf = vfs.mmap_read(&path).unwrap();
    assert!(!mf.is_stale());

    // Rewrite same-size content to change mtime but keep size at 5.
    std::thread::sleep(std::time::Duration::from_millis(10));
    vfs.write(&path, b"world").unwrap();

    assert!(mf.is_stale(), "stale: mtime changed, size same");
    vfs.delete(&path).unwrap();
}

#[test]
#[allow(unsafe_code)]
fn is_stale_size_changed_only() {
    use std::{env, fs};

    let path = env::temp_dir().join("reovim_stale_size_only.txt");
    fs::write(&path, b"hello").unwrap();
    let original_mtime = fs::metadata(&path).unwrap().modified().unwrap();

    // Construct a MappedFile with the file's real mtime but a wrong size.
    let file = fs::File::open(&path).unwrap();
    // SAFETY: read-only mmap of a test file we control.
    let mmap = unsafe { memmap2::Mmap::map(&file) }.unwrap();
    let mf = MappedFile::new(mmap, &path, original_mtime, 999);
    drop(file);

    assert!(mf.is_stale(), "stale: mtime same, size differs");
    fs::remove_file(&path).ok();
}
