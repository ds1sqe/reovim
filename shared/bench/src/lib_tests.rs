use {super::*, reovim_driver_vfs::VfsDriver};

#[test]
fn test_fixture_flat_creates_files() {
    let fixture = fixtures::TreeFixture::flat(10);
    assert_eq!(fixture.root.to_str().unwrap(), "/bench");
    // MockVfs should have 10 files
    let entries = fixture
        .vfs
        .list_dir(&fixture.root)
        .expect("root dir should exist");
    assert_eq!(entries.len(), 10);
}

#[test]
fn test_fixture_flat_zero_files() {
    let fixture = fixtures::TreeFixture::flat(0);
    let entries = fixture
        .vfs
        .list_dir(&fixture.root)
        .expect("root dir should exist");
    assert_eq!(entries.len(), 0);
}

#[test]
fn test_fixture_nested() {
    let fixture = fixtures::TreeFixture::nested(3, 2);
    // Root should exist
    let entries = fixture
        .vfs
        .list_dir(&fixture.root)
        .expect("root dir should exist");
    // Root has 2 files + 1 subdirectory
    assert_eq!(entries.len(), 3);
}

#[test]
fn test_fixture_project() {
    let fixture = fixtures::TreeFixture::project(30);
    let entries = fixture
        .vfs
        .list_dir(&fixture.root)
        .expect("root dir should exist");
    // Should have src/, tests/, docs/ subdirectories
    assert!(!entries.is_empty());
}

#[test]
fn test_scaling_constants() {
    assert_eq!(scaling::SIZES_SMALL.len(), 3);
    assert_eq!(scaling::SIZES_MEDIUM.len(), 3);
    assert_eq!(scaling::SIZES_LARGE.len(), 3);

    // Each set should be strictly increasing
    for sizes in [
        scaling::SIZES_SMALL,
        scaling::SIZES_MEDIUM,
        scaling::SIZES_LARGE,
    ] {
        for window in sizes.windows(2) {
            assert!(window[0] < window[1]);
        }
    }
}
