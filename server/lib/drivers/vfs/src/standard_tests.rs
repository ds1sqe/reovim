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

    let _ = vfs.remove_dir_all(&dir); // clean up any leftover from previous runs
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

#[test]
fn test_standard_vfs_default() {
    let vfs = StandardVfs;
    assert!(!vfs.exists(Path::new("/nonexistent_xyz_abc")));
}

#[test]
fn test_standard_vfs_init_shutdown() {
    let mut vfs = StandardVfs::new();
    vfs.init().unwrap();
    vfs.shutdown().unwrap();
}

#[test]
fn test_standard_vfs_write_str() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_write_str.txt");

    vfs.write_str(&path, "hello string").unwrap();
    let content = vfs.read_to_string(&path).unwrap();
    assert_eq!(content, "hello string");

    vfs.delete(&path).unwrap();
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_standard_vfs_metadata_directory() {
    let vfs = StandardVfs::new();
    let dir = env::temp_dir().join("reovim_test_vfs_meta_dir");

    if !vfs.exists(&dir) {
        vfs.create_dir(&dir).unwrap();
    }

    let meta = vfs.metadata(&dir).unwrap();
    assert!(meta.is_dir);
    assert!(!meta.is_file);
    assert!(meta.modified.is_some());

    vfs.remove_dir(&dir).unwrap();
}

#[test]
fn test_standard_vfs_symlink_metadata() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_symlink_meta.txt");

    vfs.write(&path, b"test data").unwrap();
    let meta = vfs.symlink_metadata(&path).unwrap();
    assert!(meta.is_file);
    assert_eq!(meta.size, 9);

    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_vfs_canonicalize() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_canon.txt");

    vfs.write(&path, b"data").unwrap();
    let canonical = vfs.canonicalize(&path).unwrap();
    assert!(canonical.is_absolute());
    assert!(canonical.exists());

    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_vfs_canonicalize_not_found() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_nonexistent_canon_xyz");
    let result = vfs.canonicalize(&path);
    assert!(result.is_err());
}

#[test]
fn test_standard_vfs_create_dir_all() {
    let vfs = StandardVfs::new();
    let dir = env::temp_dir().join("reovim_test_vfs_dir_all/nested/deep");

    vfs.create_dir_all(&dir).unwrap();
    assert!(vfs.exists(&dir));

    let meta = vfs.metadata(&dir).unwrap();
    assert!(meta.is_dir);

    vfs.remove_dir_all(&env::temp_dir().join("reovim_test_vfs_dir_all"))
        .unwrap();
}

#[test]
fn test_standard_vfs_remove_dir() {
    let vfs = StandardVfs::new();
    let dir = env::temp_dir().join("reovim_test_vfs_rmdir");

    vfs.create_dir(&dir).unwrap();
    assert!(vfs.exists(&dir));

    vfs.remove_dir(&dir).unwrap();
    assert!(!vfs.exists(&dir));
}

#[test]
fn test_standard_vfs_remove_dir_all() {
    let vfs = StandardVfs::new();
    let dir = env::temp_dir().join("reovim_test_vfs_rmdir_all");

    vfs.create_dir_all(&dir.join("sub")).unwrap();
    vfs.write(&dir.join("sub/file.txt"), b"data").unwrap();

    vfs.remove_dir_all(&dir).unwrap();
    assert!(!vfs.exists(&dir));
}

#[test]
fn test_standard_file_handle_metadata() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_fh_meta.txt");

    vfs.write(&path, b"file content").unwrap();

    let handle = vfs.open(&path, OpenOptions::read()).unwrap();
    let meta = handle.metadata().unwrap();
    assert!(meta.is_file);
    assert_eq!(meta.size, 12);

    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_file_handle_sync_all() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_fh_sync.txt");

    let mut handle = vfs.open(&path, OpenOptions::write()).unwrap();
    handle.write_all(b"sync data").unwrap();
    handle.sync_all().unwrap();

    drop(handle);
    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_file_handle_read_to_end() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_fh_read_end.txt");

    vfs.write(&path, b"all the content here").unwrap();

    let mut handle = vfs.open(&path, OpenOptions::read()).unwrap();
    let mut buf = Vec::new();
    let n = handle.read_to_end(&mut buf).unwrap();
    assert_eq!(n, 20);
    assert_eq!(&buf, b"all the content here");

    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_file_handle_read_exact() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_fh_exact.txt");

    vfs.write(&path, b"exactdata").unwrap();

    let mut handle = vfs.open(&path, OpenOptions::read()).unwrap();
    let mut buf = [0u8; 5];
    handle.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"exact");

    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_file_handle_seek_end() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_fh_seek_end.txt");

    vfs.write(&path, b"abcde").unwrap();

    let mut handle = vfs.open(&path, OpenOptions::read()).unwrap();
    let pos = handle.seek(SeekFrom::End(0)).unwrap();
    assert_eq!(pos, 5);
    assert_eq!(handle.position(), 5);

    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_file_handle_seek_current() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_fh_seek_cur.txt");

    vfs.write(&path, b"abcdefgh").unwrap();

    let mut handle = vfs.open(&path, OpenOptions::read()).unwrap();
    let mut buf = [0u8; 3];
    handle.read_exact(&mut buf).unwrap();
    assert_eq!(handle.position(), 3);

    handle.seek(SeekFrom::Current(2)).unwrap();
    assert_eq!(handle.position(), 5);

    handle.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"fgh");

    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_file_handle_path() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_fh_path.txt");

    vfs.write(&path, b"test").unwrap();

    let handle = vfs.open(&path, OpenOptions::read()).unwrap();
    assert_eq!(handle.path(), path.as_path());

    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_file_handle_size() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_fh_size.txt");

    vfs.write(&path, b"size check").unwrap();

    let handle = vfs.open(&path, OpenOptions::read()).unwrap();
    let size = handle.size().unwrap();
    assert_eq!(size, 10);

    vfs.delete(&path).unwrap();
}

#[test]
fn test_standard_vfs_read_link_nonexistent() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_nonexistent_link_xyz");
    let result = vfs.read_link(&path);
    assert!(result.is_err());
}

#[test]
fn test_standard_file_handle_append_mode() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_append.txt");

    vfs.write(&path, b"first").unwrap();

    {
        let mut handle = vfs.open(&path, OpenOptions::append()).unwrap();
        handle.write_all(b" second").unwrap();
        handle.flush().unwrap();
    }

    let content = vfs.read_to_string(&path).unwrap();
    assert_eq!(content, "first second");

    vfs.delete(&path).unwrap();
}

#[test]
fn test_metadata_from_std_times() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_meta_times.txt");

    vfs.write(&path, b"content").unwrap();
    let meta = vfs.metadata(&path).unwrap();

    // On most systems, modified time should be available
    assert!(meta.modified.is_some());
    // created may or may not be available depending on platform
    // accessed should generally be available
    assert!(meta.accessed.is_some());

    vfs.delete(&path).unwrap();
}

#[cfg(unix)]
#[test]
fn test_metadata_symlink() {
    let vfs = StandardVfs::new();
    let target = env::temp_dir().join("reovim_test_vfs_symlink_target.txt");
    let link = env::temp_dir().join("reovim_test_vfs_symlink_link.txt");

    // Cleanup in case of previous failed test
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_file(&target);

    vfs.write(&target, b"target content").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    // symlink_metadata should report is_symlink = true
    let meta = vfs.symlink_metadata(&link).unwrap();
    assert!(meta.is_symlink);

    // read_link should resolve the symlink
    let resolved = vfs.read_link(&link).unwrap();
    assert_eq!(resolved, target);

    // Cleanup
    let _ = std::fs::remove_file(&link);
    vfs.delete(&target).unwrap();
}

#[cfg(unix)]
#[test]
fn test_metadata_unix_permissions() {
    let vfs = StandardVfs::new();
    let path = env::temp_dir().join("reovim_test_vfs_unix_perms.txt");

    vfs.write(&path, b"content").unwrap();
    let meta = vfs.metadata(&path).unwrap();

    // File should be readable and writable by owner
    assert!(meta.permissions.is_readable());
    assert!(meta.permissions.is_writable());

    vfs.delete(&path).unwrap();
}
