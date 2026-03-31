//! Tests for `MappedFile`.

use std::sync::Arc;

use reovim_kernel::api::v1::FileMapping;

use crate::MappedFile;

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
    use crate::{StandardVfs, VfsDriver};
    use std::env;

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
