use {super::*, reovim_driver_vfs::VfsDriver, std::path::Path};

#[test]
fn state_create_defaults() {
    let state = ExplorerState::create();
    assert!(!state.active);
    assert_eq!(state.cursor_index, 0);
    assert_eq!(state.scroll_offset, 0);
    assert_eq!(state.visible_height, 24);
    assert_eq!(state.width, 30);
    assert!(!state.show_hidden);
    assert_eq!(state.input_mode, ExplorerInputMode::None);
    assert!(state.input_buffer.is_empty());
    assert!(state.message.is_none());
    assert_eq!(state.root_path, PathBuf::new());
    assert!(state.tree.is_none());
}

#[test]
fn state_debug() {
    let state = ExplorerState::create();
    let debug = format!("{state:?}");
    assert!(debug.contains("ExplorerState"));
}

#[test]
fn text_input_sink_insert_char() {
    let mut state = ExplorerState::create();
    state.active = true;
    state.input_mode = ExplorerInputMode::CreateFile;

    TextInputSink::insert_char(&mut state, 'h');
    TextInputSink::insert_char(&mut state, 'i');
    assert_eq!(state.input_buffer, "hi");
}

#[test]
fn text_input_sink_unicode() {
    let mut state = ExplorerState::create();
    state.active = true;
    state.input_mode = ExplorerInputMode::CreateFile;

    TextInputSink::insert_char(&mut state, '日');
    assert_eq!(state.input_buffer, "日");
}

#[test]
fn as_text_input_sink_active_with_input_mode() {
    let mut state = ExplorerState::create();
    state.active = true;
    state.input_mode = ExplorerInputMode::CreateFile;
    assert!(SessionExtension::as_text_input_sink(&mut state).is_some());
}

#[test]
fn as_text_input_sink_active_without_input_mode() {
    let mut state = ExplorerState::create();
    state.active = true;
    assert!(SessionExtension::as_text_input_sink(&mut state).is_none());
}

#[test]
fn as_text_input_sink_inactive() {
    let mut state = ExplorerState::create();
    assert!(SessionExtension::as_text_input_sink(&mut state).is_none());
}

#[test]
fn input_mode_labels() {
    assert_eq!(ExplorerInputMode::None.label(), "");
    assert_eq!(ExplorerInputMode::CreateFile.label(), "New file: ");
    assert_eq!(ExplorerInputMode::CreateDir.label(), "New dir: ");
    assert_eq!(ExplorerInputMode::Rename.label(), "Rename: ");
    assert_eq!(ExplorerInputMode::ConfirmDelete.label(), "Delete? (y/n): ");
}

#[test]
fn input_mode_equality() {
    assert_eq!(ExplorerInputMode::None, ExplorerInputMode::None);
    assert_ne!(ExplorerInputMode::None, ExplorerInputMode::CreateFile);
    assert_ne!(ExplorerInputMode::CreateFile, ExplorerInputMode::CreateDir);
    assert_ne!(ExplorerInputMode::Rename, ExplorerInputMode::ConfirmDelete);
}

#[test]
fn input_mode_debug() {
    let debug = format!("{:?}", ExplorerInputMode::CreateFile);
    assert!(debug.contains("CreateFile"));
}

#[test]
fn input_mode_clone() {
    let mode = ExplorerInputMode::Rename;
    #[allow(clippy::clone_on_copy)]
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
}

#[test]
fn input_mode_copy() {
    let mode = ExplorerInputMode::ConfirmDelete;
    let copied = mode;
    assert_eq!(mode, copied);
}

#[test]
fn node_count_no_tree() {
    let state = ExplorerState::create();
    assert_eq!(state.node_count(), 0);
}

#[test]
fn node_count_with_tree() {
    use {reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/root")).unwrap();
    mock.write(Path::new("/root/a.txt"), b"a").unwrap();
    mock.write(Path::new("/root/b.txt"), b"b").unwrap();

    let mut state = ExplorerState::create();
    state.tree = Some(FileTree::new(PathBuf::from("/root"), mock.as_ref()).unwrap());
    // root + 2 files
    assert_eq!(state.node_count(), 3);
}

#[test]
fn update_scroll_no_tree() {
    let mut state = ExplorerState::create();
    state.cursor_index = 5;
    state.scroll_offset = 3;
    state.update_scroll();
    assert_eq!(state.cursor_index, 0);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn update_scroll_clamps_cursor() {
    use {reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/root")).unwrap();
    mock.write(Path::new("/root/a.txt"), b"a").unwrap();

    let mut state = ExplorerState::create();
    state.tree = Some(FileTree::new(PathBuf::from("/root"), mock.as_ref()).unwrap());
    state.cursor_index = 100;
    state.visible_height = 10;
    state.update_scroll();
    assert_eq!(state.cursor_index, 1); // root + 1 file = 2 nodes, last index is 1
}

#[test]
fn update_scroll_scrolls_down() {
    use {reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/root")).unwrap();
    for i in 0..20 {
        mock.write(Path::new(&format!("/root/{i:02}.txt")), b"x")
            .unwrap();
    }

    let mut state = ExplorerState::create();
    state.tree = Some(FileTree::new(PathBuf::from("/root"), mock.as_ref()).unwrap());
    state.visible_height = 5;
    state.cursor_index = 10;
    state.update_scroll();
    // scroll_offset should bring cursor into view
    assert!(state.scroll_offset + 5 > state.cursor_index);
}

#[test]
fn update_scroll_zero_height() {
    use {reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/root")).unwrap();
    mock.write(Path::new("/root/a.txt"), b"a").unwrap();

    let mut state = ExplorerState::create();
    state.tree = Some(FileTree::new(PathBuf::from("/root"), mock.as_ref()).unwrap());
    state.visible_height = 0;
    state.cursor_index = 1;
    state.update_scroll();
    // Should not panic, cursor stays valid
    assert_eq!(state.cursor_index, 1);
}

#[test]
fn update_scroll_scrolls_up() {
    use {reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/root")).unwrap();
    for i in 0..20 {
        mock.write(Path::new(&format!("/root/{i:02}.txt")), b"x")
            .unwrap();
    }

    let mut state = ExplorerState::create();
    state.tree = Some(FileTree::new(PathBuf::from("/root"), mock.as_ref()).unwrap());
    state.visible_height = 5;
    state.scroll_offset = 10;
    state.cursor_index = 2;
    state.update_scroll();
    assert_eq!(state.scroll_offset, 2);
}

#[test]
fn node_count_uses_cache() {
    use {reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/root")).unwrap();
    mock.write(Path::new("/root/a.txt"), b"a").unwrap();
    mock.write(Path::new("/root/b.txt"), b"b").unwrap();

    let state = ExplorerState::create();
    // No tree: cache should store 0
    assert_eq!(state.node_count(), 0);
    assert_eq!(*state.cached_visible_count.lock().unwrap(), Some(0));

    // Second call returns cached value
    assert_eq!(state.node_count(), 0);
}

#[test]
fn node_count_cache_populated_with_tree() {
    use {reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/root")).unwrap();
    mock.write(Path::new("/root/a.txt"), b"a").unwrap();

    let mut state = ExplorerState::create();
    state.tree = Some(FileTree::new(PathBuf::from("/root"), mock.as_ref()).unwrap());

    // Cache starts empty
    assert!(state.cached_visible_count.lock().unwrap().is_none());

    // First call computes + caches
    assert_eq!(state.node_count(), 2);
    assert_eq!(*state.cached_visible_count.lock().unwrap(), Some(2));
}

#[test]
fn invalidate_clears_cache() {
    use {reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/root")).unwrap();
    mock.write(Path::new("/root/a.txt"), b"a").unwrap();

    let mut state = ExplorerState::create();
    state.tree = Some(FileTree::new(PathBuf::from("/root"), mock.as_ref()).unwrap());

    // Populate caches
    assert_eq!(state.node_count(), 2);
    state.set_cached_nodes_json(vec![serde_json::json!({"test": true})]);
    assert!(state.cached_nodes_json().is_some());

    // Invalidate
    state.invalidate_tree_cache();
    assert!(state.cached_visible_count.lock().unwrap().is_none());
    assert!(state.cached_nodes_json().is_none());
}

#[test]
fn update_scroll_with_count_same_as_update_scroll() {
    use {reovim_driver_vfs::MockVfs, std::sync::Arc};

    let mock = Arc::new(MockVfs::new());
    mock.create_dir(Path::new("/root")).unwrap();
    for i in 0..20 {
        mock.write(Path::new(&format!("/root/{i:02}.txt")), b"x")
            .unwrap();
    }

    // Test with update_scroll
    let mut state1 = ExplorerState::create();
    state1.tree = Some(FileTree::new(PathBuf::from("/root"), mock.as_ref()).unwrap());
    state1.visible_height = 5;
    state1.cursor_index = 15;
    state1.update_scroll();

    // Test with update_scroll_with_count
    let mut state2 = ExplorerState::create();
    state2.tree = Some(FileTree::new(PathBuf::from("/root"), mock.as_ref()).unwrap());
    state2.visible_height = 5;
    state2.cursor_index = 15;
    let count = state2.node_count();
    state2.update_scroll_with_count(count);

    assert_eq!(state1.cursor_index, state2.cursor_index);
    assert_eq!(state1.scroll_offset, state2.scroll_offset);
}

#[test]
fn invalidate_bumps_tree_generation() {
    let state = ExplorerState::create();
    assert_eq!(state.tree_generation(), 0);

    state.invalidate_tree_cache();
    assert_eq!(state.tree_generation(), 1);

    state.invalidate_tree_cache();
    assert_eq!(state.tree_generation(), 2);
}

#[test]
fn snapshot_generation_starts_at_max() {
    let state = ExplorerState::create();
    assert_eq!(state.snapshot_generation(), u64::MAX);
}

#[test]
fn set_snapshot_generation_round_trip() {
    let state = ExplorerState::create();
    state.set_snapshot_generation(42);
    assert_eq!(state.snapshot_generation(), 42);
}

#[test]
fn reset_snapshot_generation() {
    let state = ExplorerState::create();
    // Initial value is u64::MAX
    assert_eq!(state.snapshot_generation(), u64::MAX);

    // Set to a known value
    state.set_snapshot_generation(42);
    assert_eq!(state.snapshot_generation(), 42);

    // Reset should go back to u64::MAX
    state.reset_snapshot_generation();
    assert_eq!(state.snapshot_generation(), u64::MAX);
}

#[test]
fn cached_nodes_json_round_trip() {
    let state = ExplorerState::create();

    // Initially empty
    assert!(state.cached_nodes_json().is_none());

    // Set and get
    let nodes = vec![
        serde_json::json!({"name": "file.rs"}),
        serde_json::json!({"name": "dir"}),
    ];
    state.set_cached_nodes_json(nodes);
    let cached = state.cached_nodes_json().unwrap();
    assert_eq!(cached.len(), 2);
    assert_eq!(cached[0]["name"], "file.rs");
    assert_eq!(cached[1]["name"], "dir");
}
