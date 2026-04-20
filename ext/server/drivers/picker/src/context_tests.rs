use super::*;

#[test]
fn context_construction() {
    let ctx = PickerContext {
        cwd: PathBuf::from("/home/user/project"),
        query: "main".to_owned(),
        buffers: vec![],
        commands: vec![],
        options: vec![],
    };
    assert_eq!(ctx.cwd, PathBuf::from("/home/user/project"));
    assert_eq!(ctx.query, "main");
    assert!(ctx.buffers.is_empty());
    assert!(ctx.commands.is_empty());
}

#[test]
fn context_with_buffers() {
    let ctx = PickerContext {
        cwd: PathBuf::from("."),
        query: String::new(),
        buffers: vec![
            BufferInfo {
                id: 1,
                name: "main.rs".to_owned(),
                modified: false,
            },
            BufferInfo {
                id: 2,
                name: "lib.rs".to_owned(),
                modified: true,
            },
        ],
        commands: vec![],
        options: vec![],
    };
    assert_eq!(ctx.buffers.len(), 2);
    assert_eq!(ctx.buffers[0].name, "main.rs");
    assert!(!ctx.buffers[0].modified);
    assert!(ctx.buffers[1].modified);
}

#[test]
fn context_with_commands() {
    let ctx = PickerContext {
        cwd: PathBuf::from("."),
        query: String::new(),
        buffers: vec![],
        commands: vec![CommandInfo {
            qualified_name: "editor:save".to_owned(),
            description: "Save file".to_owned(),
        }],
        options: vec![],
    };
    assert_eq!(ctx.commands.len(), 1);
    assert_eq!(ctx.commands[0].qualified_name, "editor:save");
}

#[test]
fn context_clone() {
    let ctx = PickerContext {
        cwd: PathBuf::from("/tmp"),
        query: "test".to_owned(),
        buffers: vec![BufferInfo {
            id: 1,
            name: "a.rs".to_owned(),
            modified: false,
        }],
        commands: vec![],
        options: vec![],
    };
    #[allow(clippy::redundant_clone)]
    let cloned = ctx.clone();
    assert_eq!(cloned.query, "test");
    assert_eq!(cloned.buffers.len(), 1);
}

#[test]
fn context_debug() {
    let ctx = PickerContext {
        cwd: PathBuf::from("."),
        query: String::new(),
        buffers: vec![],
        commands: vec![],
        options: vec![],
    };
    let debug = format!("{ctx:?}");
    assert!(debug.contains("PickerContext"));
}

#[test]
fn buffer_info_debug_clone() {
    let info = BufferInfo {
        id: 5,
        name: "test.rs".to_owned(),
        modified: true,
    };
    let cloned = info.clone();
    assert_eq!(cloned.id, 5);
    let debug = format!("{info:?}");
    assert!(debug.contains("BufferInfo"));
}

#[test]
fn command_info_debug_clone() {
    let info = CommandInfo {
        qualified_name: "vim:delete".to_owned(),
        description: "Delete text".to_owned(),
    };
    let cloned = info.clone();
    assert_eq!(cloned.qualified_name, "vim:delete");
    let debug = format!("{info:?}");
    assert!(debug.contains("CommandInfo"));
}
