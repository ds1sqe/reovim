use {
    super::*,
    reovim_driver_vfs::VfsDriver,
    std::path::{Path, PathBuf},
};

#[test]
fn bridge_kind() {
    assert_eq!(ExplorerBridge.kind(), "explorer");
}

#[test]
fn bridge_scope() {
    assert_eq!(ExplorerBridge.scope(), ExtensionScope::Client);
}

#[test]
fn snapshot_no_state_returns_none() {
    let map = ExtensionMap::new();
    assert!(ExplorerBridge.snapshot(&map).is_none());
}

#[test]
fn snapshot_inactive() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<ExplorerState>();

    let snap = ExplorerBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], false);
    assert!(snap.get("cursorIndex").is_none());
}

#[test]
fn snapshot_active_defaults() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/my-project");

    let snap = ExplorerBridge.snapshot(&map).unwrap();
    assert_eq!(snap["active"], true);
    assert_eq!(snap["rootName"], "my-project");
    assert_eq!(snap["cursorIndex"], 0);
    assert_eq!(snap["scrollOffset"], 0);
    assert_eq!(snap["width"], 30);
    assert_eq!(snap["inputMode"], "none");
    assert_eq!(snap["inputBuffer"], "");
    assert_eq!(snap["showHidden"], false);
    assert!(snap.get("cutPath").is_none());
    assert!(snap["nodes"].as_array().unwrap().is_empty());
    assert!(snap.get("message").is_none());
}

#[test]
fn snapshot_active_with_message() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/root");
    state.message = Some("File created".to_owned());

    let snap = ExplorerBridge.snapshot(&map).unwrap();
    assert_eq!(snap["message"], "File created");
}

#[test]
fn snapshot_input_modes() {
    let mut map = ExtensionMap::new();
    {
        let state = map.get_or_insert::<ExplorerState>();
        state.active = true;
        state.root_path = PathBuf::from("/root");
    }

    let modes = [
        (ExplorerInputMode::None, "none"),
        (ExplorerInputMode::CreateFile, "createFile"),
        (ExplorerInputMode::CreateDir, "createDir"),
        (ExplorerInputMode::Rename, "rename"),
        (ExplorerInputMode::ConfirmDelete, "confirmDelete"),
    ];

    for (mode, expected) in modes {
        map.get_or_insert::<ExplorerState>().input_mode = mode;
        let snap = ExplorerBridge.snapshot(&map).unwrap();
        assert_eq!(snap["inputMode"], expected);
    }
}

#[test]
fn snapshot_with_cursor_and_scroll() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/root");
    state.cursor_index = 5;
    state.scroll_offset = 2;
    state.width = 40;
    state.show_hidden = true;

    let snap = ExplorerBridge.snapshot(&map).unwrap();
    assert_eq!(snap["cursorIndex"], 5);
    assert_eq!(snap["scrollOffset"], 2);
    assert_eq!(snap["width"], 40);
    assert_eq!(snap["showHidden"], true);
}

#[test]
fn snapshot_with_input_buffer() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/root");
    state.input_mode = ExplorerInputMode::CreateFile;
    state.input_buffer = "new_file.rs".to_owned();

    let snap = ExplorerBridge.snapshot(&map).unwrap();
    assert_eq!(snap["inputMode"], "createFile");
    assert_eq!(snap["inputBuffer"], "new_file.rs");
}

#[test]
fn is_active_no_state() {
    let map = ExtensionMap::new();
    assert!(!ExplorerBridge.is_active(&map));
}

#[test]
fn is_active_inactive_state() {
    let mut map = ExtensionMap::new();
    map.get_or_insert::<ExplorerState>();
    assert!(!ExplorerBridge.is_active(&map));
}

#[test]
fn is_active_active_state() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    assert!(ExplorerBridge.is_active(&map));
}

#[test]
fn snapshot_with_tree_nodes() {
    use {crate::tree::FileTree, reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/proj")).unwrap();
    mock.write(Path::new("/proj/main.rs"), b"fn main()")
        .unwrap();
    mock.create_dir(Path::new("/proj/src")).unwrap();

    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/proj");
    state.tree = Some(FileTree::new(PathBuf::from("/proj"), mock.as_ref()).unwrap());

    let snap = ExplorerBridge.snapshot(&map).unwrap();
    let nodes = snap["nodes"].as_array().unwrap();
    // Root (expanded) + src (dir) + main.rs (file) = 3 nodes
    assert_eq!(nodes.len(), 3);

    // Root node is first (depth 0)
    assert_eq!(nodes[0]["name"], "proj");
    assert!(nodes[0]["isDir"].as_bool().unwrap());

    // Directories sort before files within root's children
    assert_eq!(nodes[1]["name"], "src");
    assert!(nodes[1]["isDir"].as_bool().unwrap());
    assert_eq!(nodes[2]["name"], "main.rs");
    assert!(!nodes[2]["isDir"].as_bool().unwrap());
}

#[test]
fn snapshot_delta_omits_nodes() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/root");

    // First snapshot: includes nodes (tree_gen=0, snap_gen=MAX)
    let snap1 = ExplorerBridge.snapshot(&map).unwrap();
    assert!(snap1.get("nodes").is_some(), "first snapshot should include nodes");

    // Second snapshot: delta (tree_gen == snap_gen now)
    let snap2 = ExplorerBridge.snapshot(&map).unwrap();
    assert!(snap2.get("nodes").is_none(), "delta snapshot should omit nodes");
    assert_eq!(snap2["cursorIndex"], 0);
    assert_eq!(snap2["active"], true);
}

#[test]
fn snapshot_after_invalidate_includes_nodes() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/root");

    // First snapshot: full
    ExplorerBridge.snapshot(&map);

    // Second: delta (no nodes)
    let snap_delta = ExplorerBridge.snapshot(&map).unwrap();
    assert!(snap_delta.get("nodes").is_none());

    // Invalidate tree (simulates expand/collapse)
    map.get::<ExplorerState>().unwrap().invalidate_tree_cache();

    // Third: full again (tree_gen bumped)
    let snap_full = ExplorerBridge.snapshot(&map).unwrap();
    assert!(
        snap_full.get("nodes").is_some(),
        "snapshot after invalidate should include nodes"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn snapshot_with_symlink_node() {
    use {
        crate::tree::{
            FileTree,
            node::{FileNode, NodeType},
        },
        std::sync::Arc,
    };

    let mock = Arc::new(reovim_driver_vfs::MockVfs::new());
    mock.create_dir(Path::new("/proj")).unwrap();
    mock.write(Path::new("/proj/file.rs"), b"fn main()")
        .unwrap();

    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/proj");
    let mut tree = FileTree::new(PathBuf::from("/proj"), mock.as_ref()).unwrap();

    // Manually inject a symlink node into the tree's root children
    let root = tree.root_mut();
    if let Some(children) = root.children_mut() {
        children.push(FileNode {
            name: "link".to_string(),
            path: PathBuf::from("/proj/link"),
            node_type: NodeType::Symlink {
                target: PathBuf::from("/target"),
                broken: false,
            },
            depth: 1,
            is_hidden: false,
            is_gitignored: false,
        });
    }

    state.tree = Some(tree);

    let snap = ExplorerBridge.snapshot(&map).unwrap();
    let nodes = snap["nodes"].as_array().unwrap();

    // Find the symlink node
    let symlink_node = nodes.iter().find(|n| n["name"] == "link").unwrap();
    assert!(symlink_node["isSymlink"].as_bool().unwrap());
    assert!(!symlink_node["isDir"].as_bool().unwrap());
}

#[test]
fn snapshot_with_cut_path() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/root");
    state.cut_path = Some(PathBuf::from("/root/file.rs"));

    let snap = ExplorerBridge.snapshot(&map).unwrap();
    assert_eq!(snap["cutPath"], "/root/file.rs");
}

#[test]
fn root_name_from_path_with_no_filename() {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path = PathBuf::from("/");

    let snap = ExplorerBridge.snapshot(&map).unwrap();
    assert_eq!(snap["rootName"], "/");
}
