use {super::*, reovim_driver_vfs::MockVfs, std::path::Path};

fn setup_mock_vfs() -> MockVfs {
    let vfs = MockVfs::new();
    // /root/
    //   src/
    //     main.rs
    //     lib.rs
    //   .hidden
    //   readme.md
    vfs.add_dir("/root");
    vfs.add_dir("/root/src");
    vfs.add_file("/root/src/main.rs", "fn main() {}");
    vfs.add_file("/root/src/lib.rs", "pub mod foo;");
    vfs.add_file("/root/.hidden", "secret");
    vfs.add_file("/root/readme.md", "# Hello");
    vfs
}

#[test]
fn test_new_tree() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    assert!(tree.root().is_expanded());
    // src (dir), .hidden (file), readme.md (file)
    assert_eq!(tree.root().children().unwrap().len(), 3);
}

#[test]
fn test_root_path() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    assert_eq!(tree.root_path(), Path::new("/root"));
}

#[test]
fn test_flatten_show_all() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let flat = tree.flatten(true, true);
    // root + src + .hidden + readme.md = 4 (src is collapsed)
    assert_eq!(flat.len(), 4);
}

#[test]
fn test_flatten_hide_hidden() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let flat = tree.flatten(false, true);
    // root + src + readme.md = 3 (no .hidden)
    assert_eq!(flat.len(), 3);
}

#[test]
fn test_expand_and_flatten() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

    tree.expand(Path::new("/root/src"), &vfs).unwrap();
    let flat = tree.flatten(true, true);
    // root + src + lib.rs + main.rs + .hidden + readme.md = 6
    assert_eq!(flat.len(), 6);
}

#[test]
fn test_collapse() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

    tree.expand(Path::new("/root/src"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), 6);

    tree.collapse(Path::new("/root/src"));
    assert_eq!(tree.flatten(true, true).len(), 4);
}

#[test]
fn test_toggle() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

    // Toggle expand
    tree.toggle(Path::new("/root/src"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), 6);

    // Toggle collapse
    tree.toggle(Path::new("/root/src"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), 4);
}

#[test]
fn test_get_node() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

    let node = tree.get_node(Path::new("/root/src"));
    assert!(node.is_some());
    assert_eq!(node.unwrap().name, "src");

    let node = tree.get_node(Path::new("/nonexistent"));
    assert!(node.is_none());
}

#[test]
fn test_get_node_mut() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

    let node = tree.get_node_mut(Path::new("/root/src"));
    assert!(node.is_some());
    assert_eq!(node.unwrap().name, "src");
}

#[test]
fn test_refresh() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

    // Expand src
    tree.expand(Path::new("/root/src"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), 6);

    // Refresh should preserve expanded state
    tree.refresh(&vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), 6);
}

#[test]
fn test_expand_nonexistent_path_is_noop() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    // Expanding a nonexistent path does nothing
    tree.expand(Path::new("/nonexistent"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), 4);
}

#[test]
fn test_collapse_nonexistent_path_is_noop() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.collapse(Path::new("/nonexistent"));
    assert_eq!(tree.flatten(true, true).len(), 4);
}

#[test]
fn test_expand_file_is_noop() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.expand(Path::new("/root/readme.md"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), 4);
}

#[test]
fn test_toggle_file_is_noop() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.toggle(Path::new("/root/readme.md"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), 4);
}

#[test]
fn test_flatten_with_metadata() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.expand(Path::new("/root/src"), &vfs).unwrap();

    let flat = tree.flatten_with_metadata(true, true);
    assert_eq!(flat.len(), 6);

    // Root is always marked as last child
    assert!(flat[0].is_last_child);
    assert!(flat[0].vertical_lines.is_empty());

    // src/ should not be last (there are files after it)
    let src = flat.iter().find(|f| f.node.name == "src").unwrap();
    assert!(!src.is_last_child);
}

#[test]
fn test_flatten_with_metadata_vertical_lines() {
    let vfs = MockVfs::new();
    // /root/
    //   dir_a/
    //     file1.txt
    //   dir_b/
    //     file2.txt
    vfs.add_dir("/root");
    vfs.add_dir("/root/dir_a");
    vfs.add_file("/root/dir_a/file1.txt", "a");
    vfs.add_dir("/root/dir_b");
    vfs.add_file("/root/dir_b/file2.txt", "b");

    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.expand(Path::new("/root/dir_a"), &vfs).unwrap();
    tree.expand(Path::new("/root/dir_b"), &vfs).unwrap();

    let flat = tree.flatten_with_metadata(true, true);

    // Find file1.txt — parent dir_a has sibling (vertical_lines[0]=true),
    // file1 is last child of dir_a (vertical_lines[1]=false)
    let file1 = flat.iter().find(|f| f.node.name == "file1.txt").unwrap();
    assert_eq!(file1.vertical_lines, vec![true, false]);
    assert!(file1.is_last_child);

    // Find file2.txt — parent dir_b is last (vertical_lines[0]=false),
    // file2 is last child of dir_b (vertical_lines[1]=false)
    let file2 = flat.iter().find(|f| f.node.name == "file2.txt").unwrap();
    assert_eq!(file2.vertical_lines, vec![false, false]);
    assert!(file2.is_last_child);
}

#[test]
fn test_flatten_with_metadata_hides_hidden() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

    let flat_all = tree.flatten_with_metadata(true, true);
    let flat_visible = tree.flatten_with_metadata(false, true);

    // Hidden files should be excluded when show_hidden=false
    assert!(flat_all.len() > flat_visible.len());
    assert!(!flat_visible.iter().any(|f| f.node.is_hidden));
}

#[test]
fn test_expand_already_expanded_is_noop() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.expand(Path::new("/root/src"), &vfs).unwrap();
    let count = tree.flatten(true, true).len();
    // Expanding again should not change anything
    tree.expand(Path::new("/root/src"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), count);
}

#[test]
fn test_get_node_mut_not_found() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let node = tree.get_node_mut(Path::new("/nonexistent"));
    assert!(node.is_none());
}

#[test]
fn test_collapse_file_is_noop() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let before = tree.flatten(true, true).len();
    tree.collapse(Path::new("/root/readme.md"));
    assert_eq!(tree.flatten(true, true).len(), before);
}

#[test]
fn test_toggle_nonexistent_is_noop() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let before = tree.flatten(true, true).len();
    tree.toggle(Path::new("/nonexistent"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), before);
}

#[test]
fn test_flattened_node_clone() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let flat = tree.flatten_with_metadata(true, true);
    let cloned = flat[0].clone();
    assert_eq!(cloned.node.name, flat[0].node.name);
}

#[test]
fn test_flattened_node_debug() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let flat = tree.flatten_with_metadata(true, true);
    let debug = format!("{:?}", flat[0]);
    assert!(debug.contains("FlattenedNode"));
}

#[test]
fn test_tree_clone() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let cloned = tree.clone();
    assert_eq!(cloned.root_path(), tree.root_path());
    assert_eq!(cloned.flatten(true, true).len(), tree.flatten(true, true).len());
}

#[test]
fn test_tree_debug() {
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let debug = format!("{tree:?}");
    assert!(debug.contains("FileTree"));
}

#[test]
fn test_get_node_deep() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.expand(Path::new("/root/src"), &vfs).unwrap();

    // Should find deep node
    let node = tree.get_node(Path::new("/root/src/main.rs"));
    assert!(node.is_some());
    assert_eq!(node.unwrap().name, "main.rs");
}

#[test]
fn test_get_node_mut_deep() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.expand(Path::new("/root/src"), &vfs).unwrap();

    let node = tree.get_node_mut(Path::new("/root/src/lib.rs"));
    assert!(node.is_some());
    assert_eq!(node.unwrap().name, "lib.rs");
}

#[test]
fn test_flatten_with_metadata_hidden_at_depth() {
    // Ensure hidden node skip at depth > 0 is exercised in flatten_with_metadata
    let vfs = MockVfs::new();
    vfs.add_dir("/root");
    vfs.add_dir("/root/src");
    vfs.add_file("/root/src/.secret", "hidden");
    vfs.add_file("/root/src/visible.rs", "pub fn main() {}");

    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.expand(Path::new("/root/src"), &vfs).unwrap();

    // With show_hidden=true, .secret is included
    let flat_show = tree.flatten_with_metadata(true, true);
    assert!(flat_show.iter().any(|f| f.node.name == ".secret"));

    // With show_hidden=false, .secret at depth 2 is skipped
    let flat_hide = tree.flatten_with_metadata(false, true);
    assert!(!flat_hide.iter().any(|f| f.node.name == ".secret"));
    assert!(flat_hide.iter().any(|f| f.node.name == "visible.rs"));
}

#[test]
fn test_refresh_with_nested_expanded() {
    // Tests collect_expanded_recursive with nested expanded dirs
    let vfs = MockVfs::new();
    vfs.add_dir("/root");
    vfs.add_dir("/root/a");
    vfs.add_dir("/root/a/b");
    vfs.add_file("/root/a/b/deep.txt", "deep");

    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    tree.expand(Path::new("/root/a"), &vfs).unwrap();
    tree.expand(Path::new("/root/a/b"), &vfs).unwrap();
    assert_eq!(tree.flatten(true, true).len(), 4); // root + a + b + deep.txt

    tree.refresh(&vfs).unwrap();
    // After refresh, expanded dirs should be restored
    assert_eq!(tree.flatten(true, true).len(), 4);
}

#[test]
fn test_collapse_already_collapsed() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let before = tree.flatten(true, true).len();
    // src is already collapsed; collapsing it again should be a no-op
    tree.collapse(Path::new("/root/src"));
    assert_eq!(tree.flatten(true, true).len(), before);
}

#[test]
fn test_root_mut() {
    let vfs = setup_mock_vfs();
    let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let root = tree.root_mut();
    assert!(root.is_expanded());
}

#[test]
fn test_gitignore_marks_matching_files() {
    let vfs = MockVfs::new();
    vfs.add_dir("/proj");
    vfs.add_file("/proj/.gitignore", "target/\n*.log\n");
    vfs.add_dir("/proj/src");
    vfs.add_dir("/proj/target");
    vfs.add_file("/proj/build.log", "log data");
    vfs.add_file("/proj/main.rs", "fn main() {}");

    let tree = FileTree::new(PathBuf::from("/proj"), &vfs).unwrap();

    // With show_gitignored=true, all nodes are visible
    let all = tree.flatten(true, true);
    assert!(all.iter().any(|n| n.name == "target"));
    assert!(all.iter().any(|n| n.name == "build.log"));
    assert!(all.iter().any(|n| n.name == "main.rs"));

    // target/ and build.log should be marked as gitignored
    let target = all.iter().find(|n| n.name == "target").unwrap();
    assert!(target.is_gitignored);
    let log = all.iter().find(|n| n.name == "build.log").unwrap();
    assert!(log.is_gitignored);

    // main.rs and src/ should not be gitignored
    let main = all.iter().find(|n| n.name == "main.rs").unwrap();
    assert!(!main.is_gitignored);
    let src = all.iter().find(|n| n.name == "src").unwrap();
    assert!(!src.is_gitignored);
}

#[test]
fn test_gitignore_filters_when_hidden() {
    let vfs = MockVfs::new();
    vfs.add_dir("/proj");
    vfs.add_file("/proj/.gitignore", "*.tmp\n");
    vfs.add_file("/proj/data.tmp", "temp");
    vfs.add_file("/proj/keep.rs", "code");

    let tree = FileTree::new(PathBuf::from("/proj"), &vfs).unwrap();

    // show_gitignored=false hides gitignored files
    let filtered = tree.flatten(true, false);
    assert!(!filtered.iter().any(|n| n.name == "data.tmp"));
    assert!(filtered.iter().any(|n| n.name == "keep.rs"));
}

#[test]
fn test_gitignore_no_gitignore_file() {
    // When no .gitignore exists, nothing is marked
    let vfs = setup_mock_vfs();
    let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
    let all = tree.flatten(true, true);
    assert!(all.iter().all(|n| !n.is_gitignored));
}

#[test]
fn test_gitignore_expand_marks_children() {
    let vfs = MockVfs::new();
    vfs.add_dir("/proj");
    vfs.add_file("/proj/.gitignore", "*.o\n");
    vfs.add_dir("/proj/build");
    vfs.add_file("/proj/build/main.o", "binary");
    vfs.add_file("/proj/build/lib.rs", "code");

    let mut tree = FileTree::new(PathBuf::from("/proj"), &vfs).unwrap();
    tree.expand(Path::new("/proj/build"), &vfs).unwrap();

    let all = tree.flatten(true, true);
    let main_o = all.iter().find(|n| n.name == "main.o").unwrap();
    assert!(main_o.is_gitignored);
    let lib_rs = all.iter().find(|n| n.name == "lib.rs").unwrap();
    assert!(!lib_rs.is_gitignored);
}

#[test]
fn test_gitignore_refresh_rebuilds() {
    let vfs = MockVfs::new();
    vfs.add_dir("/proj");
    vfs.add_file("/proj/.gitignore", "*.tmp\n");
    vfs.add_file("/proj/a.tmp", "temp");
    vfs.add_file("/proj/keep.rs", "code");

    let mut tree = FileTree::new(PathBuf::from("/proj"), &vfs).unwrap();
    assert!(tree.flatten(true, true).iter().any(|n| n.is_gitignored));

    // Update .gitignore to not ignore anything
    vfs.add_file("/proj/.gitignore", "# empty\n");
    tree.refresh(&vfs).unwrap();

    // After refresh, nothing should be gitignored
    assert!(tree.flatten(true, true).iter().all(|n| !n.is_gitignored));
}

#[test]
fn test_gitignore_flatten_with_metadata_filters() {
    let vfs = MockVfs::new();
    vfs.add_dir("/proj");
    vfs.add_file("/proj/.gitignore", "ignored.txt\n");
    vfs.add_file("/proj/ignored.txt", "data");
    vfs.add_file("/proj/visible.rs", "code");

    let tree = FileTree::new(PathBuf::from("/proj"), &vfs).unwrap();

    // show_gitignored=true includes ignored file
    let all = tree.flatten_with_metadata(true, true);
    assert!(all.iter().any(|f| f.node.name == "ignored.txt"));

    // show_gitignored=false hides it
    let filtered = tree.flatten_with_metadata(true, false);
    assert!(!filtered.iter().any(|f| f.node.name == "ignored.txt"));
    assert!(filtered.iter().any(|f| f.node.name == "visible.rs"));
}
