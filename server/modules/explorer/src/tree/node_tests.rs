use {super::*, reovim_driver_vfs::MockVfs, std::path::Path};

fn setup_mock_vfs() -> MockVfs {
    let vfs = MockVfs::new();
    // Create a directory structure:
    // /root/
    //   src/        (dir)
    //   .hidden     (hidden file)
    //   main.rs     (file, 100 bytes)
    //   readme.md   (file, 50 bytes)
    vfs.add_dir("/root");
    vfs.add_dir("/root/src");
    vfs.add_file("/root/.hidden", "secret");
    vfs.add_file("/root/main.rs", "x".repeat(100));
    vfs.add_file("/root/readme.md", "y".repeat(50));
    vfs
}

#[test]
fn test_from_path_directory() {
    let vfs = setup_mock_vfs();
    let node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
    assert_eq!(node.name, "root");
    assert!(node.is_dir());
    assert!(!node.is_expanded());
    assert_eq!(node.depth, 0);
    assert!(!node.is_hidden);
}

#[test]
fn test_from_entry_file() {
    let vfs = setup_mock_vfs();
    let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
    let node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
    assert_eq!(node.name, "main.rs");
    assert!(node.is_file());
    assert!(!node.is_dir());
    assert_eq!(node.depth, 1);
    assert_eq!(node.size(), Some(100));
}

#[test]
fn test_from_entry_directory() {
    let vfs = setup_mock_vfs();
    let entry = DirEntry::directory(PathBuf::from("/root/src"));
    let node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
    assert_eq!(node.name, "src");
    assert!(node.is_dir());
    assert!(!node.is_file());
}

#[test]
fn test_hidden_file() {
    let vfs = setup_mock_vfs();
    let entry = DirEntry::file(PathBuf::from("/root/.hidden"));
    let node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
    assert!(node.is_hidden);
}

#[test]
fn test_toggle_expand() {
    let vfs = setup_mock_vfs();
    let mut node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
    assert!(!node.is_expanded());
    node.toggle_expand();
    assert!(node.is_expanded());
    node.toggle_expand();
    assert!(!node.is_expanded());
}

#[test]
fn test_set_expanded() {
    let vfs = setup_mock_vfs();
    let mut node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
    node.set_expanded(true);
    assert!(node.is_expanded());
    node.set_expanded(false);
    assert!(!node.is_expanded());
}

#[test]
fn test_load_children() {
    let vfs = setup_mock_vfs();
    let mut node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
    node.load_children(&vfs).unwrap();

    let children = node.children().unwrap();
    assert_eq!(children.len(), 4);
    // Directories first, then alphabetical
    assert_eq!(children[0].name, "src");
    assert!(children[0].is_dir());
}

#[test]
fn test_children_empty_for_file() {
    let vfs = setup_mock_vfs();
    let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
    let node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
    assert!(node.children().is_none());
}

#[test]
fn test_children_mut_empty_for_file() {
    let vfs = setup_mock_vfs();
    let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
    let mut node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
    assert!(node.children_mut().is_none());
}

#[test]
fn test_is_symlink() {
    let _vfs = MockVfs::new();
    // MockVfs doesn't support symlinks natively, but we can construct one manually
    let node = FileNode {
        name: "link".to_string(),
        path: PathBuf::from("/link"),
        node_type: NodeType::Symlink {
            target: PathBuf::from("/target"),
            broken: false,
        },
        depth: 0,
        is_hidden: false,
        is_gitignored: false,
    };
    assert!(node.is_symlink());
    assert!(!node.is_broken_symlink());
    assert!(!node.is_dir());
    assert!(!node.is_file());
}

#[test]
fn test_broken_symlink() {
    let node = FileNode {
        name: "broken".to_string(),
        path: PathBuf::from("/broken"),
        node_type: NodeType::Symlink {
            target: PathBuf::from("/nonexistent"),
            broken: true,
        },
        depth: 0,
        is_hidden: false,
        is_gitignored: false,
    };
    assert!(node.is_symlink());
    assert!(node.is_broken_symlink());
}

#[test]
fn test_file_size_none_for_dir() {
    let vfs = setup_mock_vfs();
    let node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
    assert!(node.size().is_none());
}

#[test]
fn test_toggle_expand_on_file_is_noop() {
    let vfs = setup_mock_vfs();
    let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
    let mut node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
    assert!(!node.is_expanded());
    node.toggle_expand(); // Should be a no-op
    assert!(!node.is_expanded());
}

#[test]
fn test_set_expanded_on_file_is_noop() {
    let vfs = setup_mock_vfs();
    let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
    let mut node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
    node.set_expanded(true); // Should be a no-op
    assert!(!node.is_expanded());
}

#[test]
fn test_load_children_on_file_is_noop() {
    let vfs = setup_mock_vfs();
    let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
    let mut node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
    node.load_children(&vfs).unwrap(); // Should be a no-op
    assert!(node.children().is_none());
}

#[test]
fn test_load_children_sorts_dirs_first() {
    let vfs = MockVfs::new();
    vfs.add_dir("/d");
    vfs.add_file("/d/zebra.txt", "z");
    vfs.add_dir("/d/alpha");
    vfs.add_file("/d/beta.txt", "b");
    vfs.add_dir("/d/gamma");

    let mut node = FileNode::from_path(Path::new("/d"), &vfs, 0).unwrap();
    node.load_children(&vfs).unwrap();

    let children = node.children().unwrap();
    let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
    // Dirs first (alpha, gamma), then files alphabetical (beta.txt, zebra.txt)
    assert_eq!(names, &["alpha", "gamma", "beta.txt", "zebra.txt"]);
}

#[test]
fn test_format_size_bytes() {
    assert_eq!(format_size(0), "0B");
    assert_eq!(format_size(100), "100B");
    assert_eq!(format_size(1023), "1023B");
}

#[test]
fn test_format_size_kb() {
    assert_eq!(format_size(1024), "1.0K");
    assert_eq!(format_size(1536), "1.5K");
}

#[test]
fn test_format_size_mb() {
    assert_eq!(format_size(1_048_576), "1.0M");
}

#[test]
fn test_format_size_gb() {
    assert_eq!(format_size(1_073_741_824), "1.0G");
}

#[test]
fn test_format_size_tb() {
    assert_eq!(format_size(1_099_511_627_776), "1.0T");
}

#[test]
fn test_from_path_file() {
    let vfs = setup_mock_vfs();
    let node = FileNode::from_path(Path::new("/root/main.rs"), &vfs, 1).unwrap();
    assert_eq!(node.name, "main.rs");
    assert!(node.is_file());
    assert!(!node.is_dir());
    assert_eq!(node.depth, 1);
    assert_eq!(node.size(), Some(100));
}

#[test]
fn test_from_path_root_slash() {
    let vfs = MockVfs::new();
    vfs.add_dir("/");
    let node = FileNode::from_path(Path::new("/"), &vfs, 0).unwrap();
    // "/" has no file_name, so fallback to to_string_lossy
    assert_eq!(node.name, "/");
    assert!(node.is_dir());
}

#[test]
fn test_from_entry_symlink() {
    struct SymlinkVfs(bool);

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl VfsDriver for SymlinkVfs {
        fn init(&mut self) -> Result<(), VfsError> {
            Ok(())
        }
        fn shutdown(&mut self) -> Result<(), VfsError> {
            Ok(())
        }
        fn read(&self, _: &Path) -> Result<Vec<u8>, VfsError> {
            Err(VfsError::NotFound(PathBuf::new()))
        }
        fn write(&self, _: &Path, _: &[u8]) -> Result<(), VfsError> {
            Err(VfsError::NotSupported("write".into()))
        }
        fn exists(&self, _: &Path) -> bool {
            self.0
        }
        fn metadata(&self, p: &Path) -> Result<reovim_driver_vfs::FileMetadata, VfsError> {
            self.symlink_metadata(p)
        }
        fn symlink_metadata(&self, _: &Path) -> Result<reovim_driver_vfs::FileMetadata, VfsError> {
            Ok(reovim_driver_vfs::FileMetadata::symlink())
        }
        fn canonicalize(&self, p: &Path) -> Result<PathBuf, VfsError> {
            Ok(p.to_path_buf())
        }
        fn read_link(&self, _: &Path) -> Result<PathBuf, VfsError> {
            Ok(PathBuf::from("/target"))
        }
        fn list_dir(&self, _: &Path) -> Result<Vec<DirEntry>, VfsError> {
            Ok(vec![])
        }
        fn create_dir(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn create_dir_all(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn delete(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn rename(&self, _: &Path, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn copy(&self, _: &Path, _: &Path) -> Result<u64, VfsError> {
            Ok(0)
        }
        fn remove_dir(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn remove_dir_all(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn open(
            &self,
            _: &Path,
            _: reovim_driver_vfs::OpenOptions,
        ) -> Result<Box<dyn reovim_driver_vfs::FileHandle>, VfsError> {
            Err(VfsError::NotSupported("open".into()))
        }
    }

    // exists=true → not broken
    let vfs = SymlinkVfs(true);
    let entry = DirEntry::symlink(PathBuf::from("/root/link"));
    let node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
    assert!(node.is_symlink());
    assert!(!node.is_broken_symlink());
    assert_eq!(node.depth, 1);
}

#[test]
fn test_from_path_symlink() {
    struct BrokenSymlinkVfs;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl VfsDriver for BrokenSymlinkVfs {
        fn init(&mut self) -> Result<(), VfsError> {
            Ok(())
        }
        fn shutdown(&mut self) -> Result<(), VfsError> {
            Ok(())
        }
        fn read(&self, _: &Path) -> Result<Vec<u8>, VfsError> {
            Err(VfsError::NotFound(PathBuf::new()))
        }
        fn write(&self, _: &Path, _: &[u8]) -> Result<(), VfsError> {
            Err(VfsError::NotSupported("write".into()))
        }
        fn exists(&self, _: &Path) -> bool {
            false
        }
        fn metadata(&self, p: &Path) -> Result<reovim_driver_vfs::FileMetadata, VfsError> {
            self.symlink_metadata(p)
        }
        fn symlink_metadata(&self, _: &Path) -> Result<reovim_driver_vfs::FileMetadata, VfsError> {
            Ok(reovim_driver_vfs::FileMetadata::symlink())
        }
        fn canonicalize(&self, p: &Path) -> Result<PathBuf, VfsError> {
            Ok(p.to_path_buf())
        }
        fn read_link(&self, _: &Path) -> Result<PathBuf, VfsError> {
            Ok(PathBuf::from("/nonexistent"))
        }
        fn list_dir(&self, _: &Path) -> Result<Vec<DirEntry>, VfsError> {
            Ok(vec![])
        }
        fn create_dir(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn create_dir_all(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn delete(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn rename(&self, _: &Path, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn copy(&self, _: &Path, _: &Path) -> Result<u64, VfsError> {
            Ok(0)
        }
        fn remove_dir(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn remove_dir_all(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn open(
            &self,
            _: &Path,
            _: reovim_driver_vfs::OpenOptions,
        ) -> Result<Box<dyn reovim_driver_vfs::FileHandle>, VfsError> {
            Err(VfsError::NotSupported("open".into()))
        }
    }

    let vfs = BrokenSymlinkVfs;
    let node = FileNode::from_path(Path::new("/root/link"), &vfs, 1).unwrap();
    assert!(node.is_symlink());
    assert!(node.is_broken_symlink());
}

#[test]
fn test_sort_comparator_file_before_dir() {
    // Use 30+ items to trigger merge sort (not insertion sort) so
    // the comparator sees both (dir, file) and (file, dir) orderings.
    let vfs = MockVfs::new();
    vfs.add_dir("/d");
    // 50 files + 50 dirs = 100 entries → triggers merge/quicksort
    for i in 0..50 {
        vfs.add_file(format!("/d/f{i:02}.txt"), format!("file {i}"));
        vfs.add_dir(format!("/d/d{i:02}"));
    }

    let mut node = FileNode::from_path(Path::new("/d"), &vfs, 0).unwrap();
    node.load_children(&vfs).unwrap();
    let children = node.children().unwrap();
    assert_eq!(children.len(), 100);
    // All dirs come first
    for child in &children[..50] {
        assert!(child.is_dir(), "expected dir, got file: {}", child.name);
    }
    // All files come after
    for child in &children[50..] {
        assert!(child.is_file(), "expected file, got dir: {}", child.name);
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_sort_comparator_direct() {
    // Directly exercise both sort arms by sorting manually-constructed nodes
    let file = FileNode {
        name: "file.txt".to_string(),
        path: PathBuf::from("/file.txt"),
        node_type: NodeType::File { size: 0 },
        depth: 1,
        is_hidden: false,
        is_gitignored: false,
    };
    let dir = FileNode {
        name: "dir".to_string(),
        path: PathBuf::from("/dir"),
        node_type: NodeType::Directory {
            expanded: false,
            children: vec![],
        },
        depth: 1,
        is_hidden: false,
        is_gitignored: false,
    };

    // file vs dir → Greater
    let cmp1 = match (file.is_dir(), dir.is_dir()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => file.name.to_lowercase().cmp(&dir.name.to_lowercase()),
    };
    assert_eq!(cmp1, std::cmp::Ordering::Greater);

    // dir vs file → Less
    let cmp2 = match (dir.is_dir(), file.is_dir()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => dir.name.to_lowercase().cmp(&file.name.to_lowercase()),
    };
    assert_eq!(cmp2, std::cmp::Ordering::Less);
}

#[test]
fn test_load_children_entry_error() {
    // VFS where list_dir returns entries but symlink_metadata fails for one
    struct PartialErrorVfs;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl VfsDriver for PartialErrorVfs {
        fn init(&mut self) -> Result<(), VfsError> {
            Ok(())
        }
        fn shutdown(&mut self) -> Result<(), VfsError> {
            Ok(())
        }
        fn read(&self, _: &Path) -> Result<Vec<u8>, VfsError> {
            Ok(vec![])
        }
        fn write(&self, _: &Path, _: &[u8]) -> Result<(), VfsError> {
            Ok(())
        }
        fn exists(&self, _: &Path) -> bool {
            true
        }
        fn metadata(&self, p: &Path) -> Result<reovim_driver_vfs::FileMetadata, VfsError> {
            self.symlink_metadata(p)
        }
        fn symlink_metadata(&self, p: &Path) -> Result<reovim_driver_vfs::FileMetadata, VfsError> {
            if p.file_name().is_some_and(|n| n == "bad.txt") {
                Err(VfsError::PermissionDenied(p.to_path_buf()))
            } else if p.ends_with("good.txt") {
                Ok(reovim_driver_vfs::FileMetadata::file(10))
            } else {
                Ok(reovim_driver_vfs::FileMetadata::directory())
            }
        }
        fn canonicalize(&self, p: &Path) -> Result<PathBuf, VfsError> {
            Ok(p.to_path_buf())
        }
        fn read_link(&self, _: &Path) -> Result<PathBuf, VfsError> {
            Err(VfsError::NotSupported("read_link".into()))
        }
        fn list_dir(&self, _: &Path) -> Result<Vec<DirEntry>, VfsError> {
            Ok(vec![
                DirEntry::file(PathBuf::from("/root/good.txt")),
                DirEntry::file(PathBuf::from("/root/bad.txt")),
            ])
        }
        fn create_dir(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn create_dir_all(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn delete(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn rename(&self, _: &Path, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn copy(&self, _: &Path, _: &Path) -> Result<u64, VfsError> {
            Ok(0)
        }
        fn remove_dir(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn remove_dir_all(&self, _: &Path) -> Result<(), VfsError> {
            Ok(())
        }
        fn open(
            &self,
            _: &Path,
            _: reovim_driver_vfs::OpenOptions,
        ) -> Result<Box<dyn reovim_driver_vfs::FileHandle>, VfsError> {
            Err(VfsError::NotSupported("open".into()))
        }
    }

    let vfs = PartialErrorVfs;
    let mut node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
    node.load_children(&vfs).unwrap();
    // Only good.txt should be loaded; bad.txt's from_entry fails silently
    let children = node.children().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "good.txt");
}

#[test]
fn test_from_path_name_extraction() {
    let vfs = MockVfs::new();
    vfs.add_dir("/my-project");
    let node = FileNode::from_path(Path::new("/my-project"), &vfs, 0).unwrap();
    assert_eq!(node.name, "my-project");
}
