use {super::*, reovim_driver_command::Command};

#[test]
fn toggle_metadata() {
    let cmd = Toggle;
    assert_eq!(cmd.id(), ids::TOGGLE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn close_metadata() {
    let cmd = Close;
    assert_eq!(cmd.id(), ids::CLOSE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn cursor_up_metadata() {
    let cmd = CursorUp;
    assert_eq!(cmd.id(), ids::CURSOR_UP);
    assert!(!cmd.description().is_empty());
}

#[test]
fn cursor_down_metadata() {
    let cmd = CursorDown;
    assert_eq!(cmd.id(), ids::CURSOR_DOWN);
    assert!(!cmd.description().is_empty());
}

#[test]
fn goto_first_metadata() {
    let cmd = GotoFirst;
    assert_eq!(cmd.id(), ids::GOTO_FIRST);
    assert!(!cmd.description().is_empty());
}

#[test]
fn goto_last_metadata() {
    let cmd = GotoLast;
    assert_eq!(cmd.id(), ids::GOTO_LAST);
    assert!(!cmd.description().is_empty());
}

#[test]
fn expand_metadata() {
    let cmd = Expand;
    assert_eq!(cmd.id(), ids::EXPAND);
    assert!(!cmd.description().is_empty());
}

#[test]
fn collapse_metadata() {
    let cmd = Collapse;
    assert_eq!(cmd.id(), ids::COLLAPSE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn open_metadata() {
    let cmd = Open;
    assert_eq!(cmd.id(), ids::OPEN);
    assert!(!cmd.description().is_empty());
}

#[test]
fn goto_parent_metadata() {
    let cmd = GotoParent;
    assert_eq!(cmd.id(), ids::GOTO_PARENT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn toggle_hidden_metadata() {
    let cmd = ToggleHidden;
    assert_eq!(cmd.id(), ids::TOGGLE_HIDDEN);
    assert!(!cmd.description().is_empty());
}

#[test]
fn refresh_metadata() {
    let cmd = Refresh;
    assert_eq!(cmd.id(), ids::REFRESH);
    assert!(!cmd.description().is_empty());
}

#[test]
fn create_file_metadata() {
    let cmd = CreateFile;
    assert_eq!(cmd.id(), ids::CREATE_FILE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn create_dir_metadata() {
    let cmd = CreateDir;
    assert_eq!(cmd.id(), ids::CREATE_DIR);
    assert!(!cmd.description().is_empty());
}

#[test]
fn rename_metadata() {
    let cmd = Rename;
    assert_eq!(cmd.id(), ids::RENAME);
    assert!(!cmd.description().is_empty());
}

#[test]
fn delete_metadata() {
    let cmd = Delete;
    assert_eq!(cmd.id(), ids::DELETE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn confirm_input_metadata() {
    let cmd = ConfirmInput;
    assert_eq!(cmd.id(), ids::CONFIRM_INPUT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn cancel_input_metadata() {
    let cmd = CancelInput;
    assert_eq!(cmd.id(), ids::CANCEL_INPUT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn input_backspace_metadata() {
    let cmd = InputBackspace;
    assert_eq!(cmd.id(), ids::INPUT_BACKSPACE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn yank_path_metadata() {
    let cmd = YankPath;
    assert_eq!(cmd.id(), ids::YANK_PATH);
    assert!(!cmd.description().is_empty());
}

#[test]
fn cut_mark_metadata() {
    let cmd = CutMark;
    assert_eq!(cmd.id(), ids::CUT_MARK);
    assert!(!cmd.description().is_empty());
}

#[test]
fn paste_metadata() {
    let cmd = Paste;
    assert_eq!(cmd.id(), ids::PASTE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 22);
}

#[test]
fn command_handlers_unique_ids() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    let mut deduped = ids.clone();
    deduped.sort_by_key(CommandId::name_owned);
    deduped.dedup_by_key(|id| id.name_owned());
    assert_eq!(ids.len(), deduped.len());
}

#[test]
fn toggle_debug() {
    let debug = format!("{Toggle:?}");
    assert!(debug.contains("Toggle"));
}

#[test]
fn close_debug() {
    let debug = format!("{Close:?}");
    assert!(debug.contains("Close"));
}

#[test]
fn cursor_up_debug() {
    let debug = format!("{CursorUp:?}");
    assert!(debug.contains("CursorUp"));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn all_default_constructable() {
    let _ = Toggle::default();
    let _ = Close::default();
    let _ = CursorUp::default();
    let _ = CursorDown::default();
    let _ = GotoFirst::default();
    let _ = GotoLast::default();
    let _ = Expand::default();
    let _ = Collapse::default();
    let _ = Open::default();
    let _ = GotoParent::default();
    let _ = ToggleHidden::default();
    let _ = Refresh::default();
    let _ = CreateFile::default();
    let _ = CreateDir::default();
    let _ = Rename::default();
    let _ = Delete::default();
    let _ = ConfirmInput::default();
    let _ = CancelInput::default();
    let _ = InputBackspace::default();
    let _ = YankPath::default();
    let _ = CutMark::default();
    let _ = Paste::default();
}

#[test]
fn cursor_dir_path_on_dir() {
    use {crate::tree::node::FileNode, reovim_driver_vfs::MockVfs, std::path::Path};

    let vfs = MockVfs::new();
    vfs.add_dir("/root");
    vfs.add_dir("/root/src");
    let mut root = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
    root.set_expanded(true);
    root.load_children(&vfs).unwrap();

    let nodes: Vec<&FileNode> = vec![&root];
    let result = cursor_dir_path(&nodes, 0);
    assert_eq!(result, PathBuf::from("/root"));
}

#[test]
fn cursor_dir_path_on_file() {
    use {
        crate::tree::node::{FileNode, NodeType},
        std::path::PathBuf,
    };

    let file_node = FileNode {
        name: "main.rs".to_string(),
        path: PathBuf::from("/root/main.rs"),
        node_type: NodeType::File { size: 100 },
        depth: 1,
        is_hidden: false,
        is_gitignored: false,
    };

    let nodes: Vec<&FileNode> = vec![&file_node];
    let result = cursor_dir_path(&nodes, 0);
    assert_eq!(result, PathBuf::from("/root"));
}

#[test]
fn cursor_dir_path_out_of_bounds() {
    let nodes: Vec<&crate::tree::node::FileNode> = vec![];
    let result = cursor_dir_path(&nodes, 5);
    assert_eq!(result, PathBuf::from("."));
}
