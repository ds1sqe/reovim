use {super::*, reovim_driver_vfs::VfsDriver};

#[test]
fn test_digit_width() {
    assert_eq!(digit_width(0), 1);
    assert_eq!(digit_width(1), 1);
    assert_eq!(digit_width(9), 1);
    assert_eq!(digit_width(10), 1);
    assert_eq!(digit_width(11), 2);
    assert_eq!(digit_width(100), 2);
    assert_eq!(digit_width(101), 3);
    assert_eq!(digit_width(1000), 3);
    assert_eq!(digit_width(1001), 4);
}

#[test]
fn test_flat_file_names_are_zero_padded() {
    let fixture = TreeFixture::flat(15);
    // Should have files 00.rs through 14.rs (2-digit padding)
    let entries = fixture.vfs.list_dir(&fixture.root).unwrap();
    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    assert!(names.contains(&"00.rs".to_string()));
    assert!(names.contains(&"14.rs".to_string()));
}

#[test]
fn test_nested_total_files() {
    let fixture = TreeFixture::nested(4, 3);
    // 4 levels * 3 files = 12 files total
    let count = count_files_recursive(&fixture.vfs, &fixture.root);
    assert_eq!(count, 12);
}

#[test]
fn test_nested_depth_one() {
    let fixture = TreeFixture::nested(1, 5);
    // 1 level, 5 files, no subdirs
    let entries = fixture.vfs.list_dir(&fixture.root).unwrap();
    assert_eq!(entries.len(), 5);
    assert!(entries.iter().all(|e| e.is_file));
}

#[test]
fn test_project_has_root_files() {
    let fixture = TreeFixture::project(50);
    let entries = fixture.vfs.list_dir(&fixture.root).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Cargo.toml"));
    assert!(names.contains(&"README.md"));
    assert!(names.contains(&"src"));
    assert!(names.contains(&"tests"));
    assert!(names.contains(&"docs"));
}

#[test]
fn test_project_small_no_module_dir() {
    // total_files = 1: src_count = 0, so src_mod_count = 0 (false branch of if)
    let fixture = TreeFixture::project(1);
    let entries = fixture.vfs.list_dir(&fixture.root).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Cargo.toml"));
    assert!(names.contains(&"src"));
    assert!(names.contains(&"docs"));
    // src/ should have no module/ subdir since src_count = 0
    let src_entries = fixture
        .vfs
        .list_dir(std::path::Path::new("/bench/src"))
        .unwrap();
    assert!(
        src_entries.iter().all(|e| e.name != "module"),
        "module/ directory should not exist when src_count is 0"
    );
}

#[test]
fn test_project_zero_files() {
    let fixture = TreeFixture::project(0);
    let entries = fixture.vfs.list_dir(&fixture.root).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    // Only root config files
    assert!(names.contains(&"Cargo.toml"));
    assert!(names.contains(&"README.md"));
}

/// Recursively count all files (not directories) under a path.
fn count_files_recursive(vfs: &MockVfs, path: &std::path::Path) -> usize {
    let Ok(entries) = vfs.list_dir(path) else {
        return 0;
    };
    let mut count = 0;
    for entry in &entries {
        if entry.is_file {
            count += 1;
        } else if entry.is_dir {
            count += count_files_recursive(vfs, &entry.path);
        }
    }
    count
}
