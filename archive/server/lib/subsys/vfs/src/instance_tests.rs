use {super::*, crate::MockVfs};

#[test]
fn test_vfs_instance() {
    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let instance = VfsInstance::new(Arc::clone(&vfs));

    // Should be able to get driver back
    let retrieved = instance.driver();
    assert!(Arc::ptr_eq(retrieved, &vfs));
}

#[test]
fn test_vfs_instance_debug() {
    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let instance = VfsInstance::new(vfs);

    let debug = format!("{instance:?}");
    assert!(debug.contains("VfsInstance"));
}

#[test]
fn test_vfs_instance_driver_usable() {
    use std::path::Path;

    let mock = MockVfs::new();
    mock.add_file_str("/test.txt", "hello");
    let vfs: Arc<dyn VfsDriver> = Arc::new(mock);
    let instance = VfsInstance::new(vfs);

    let driver = instance.driver();
    let content = driver.read_to_string(Path::new("/test.txt")).unwrap();
    assert_eq!(content, "hello");
}
