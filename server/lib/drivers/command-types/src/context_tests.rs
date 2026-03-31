use super::*;

#[test]
fn test_command_context_new() {
    let ctx = CommandContext::new();
    assert!(ctx.count().is_none());
    assert!(ctx.register().is_none());
    assert!(!ctx.has_bang());
}

#[test]
fn test_command_context_default() {
    let ctx = CommandContext::default();
    assert!(ctx.count().is_none());
    assert!(ctx.register().is_none());
    assert!(!ctx.has_bang());
    assert!(ctx.vfs().is_none());
    assert!(ctx.mode_name().is_none());
    assert!(ctx.cursor_position().is_none());
    assert!(ctx.buffer_id().is_none());
    assert!(ctx.window_id().is_none());
    assert!(ctx.range().is_none());
    assert!(!ctx.is_operator_pending());
    assert!(!ctx.is_linewise());
}

#[test]
fn test_command_context_count() {
    let mut ctx = CommandContext::new();
    ctx.set("count", ArgValue::Count(5));
    assert_eq!(ctx.count(), Some(5));
}

#[test]
fn test_command_context_count_zero() {
    let mut ctx = CommandContext::new();
    ctx.set("count", ArgValue::Count(0));
    assert_eq!(ctx.count(), Some(0));
}

#[test]
fn test_command_context_count_wrong_type_returns_none() {
    let mut ctx = CommandContext::new();
    ctx.set("count", ArgValue::String("5".to_string()));
    assert!(ctx.count().is_none());
}

#[test]
fn test_command_context_count_missing_returns_none() {
    let ctx = CommandContext::new();
    assert!(ctx.count().is_none());
}

#[test]
fn test_command_context_register() {
    let mut ctx = CommandContext::new();
    ctx.set("register", ArgValue::Register('a'));
    assert_eq!(ctx.register(), Some('a'));
}

#[test]
fn test_command_context_register_wrong_type_returns_none() {
    let mut ctx = CommandContext::new();
    ctx.set("register", ArgValue::Char('a'));
    assert!(ctx.register().is_none());
}

#[test]
fn test_command_context_bang() {
    let mut ctx = CommandContext::new();
    assert!(!ctx.has_bang());

    ctx.set("bang", ArgValue::Bang(true));
    assert!(ctx.has_bang());
}

#[test]
fn test_command_context_bang_false() {
    let mut ctx = CommandContext::new();
    ctx.set("bang", ArgValue::Bang(false));
    assert!(!ctx.has_bang());
}

#[test]
fn test_command_context_bang_wrong_type_returns_false() {
    let mut ctx = CommandContext::new();
    ctx.set("bang", ArgValue::String("true".to_string()));
    assert!(!ctx.has_bang());
}

#[test]
fn test_command_context_string() {
    let mut ctx = CommandContext::new();
    ctx.set("file", ArgValue::String("test.txt".into()));
    assert_eq!(ctx.string("file"), Some("test.txt"));
}

#[test]
fn test_command_context_string_from_file_path() {
    let mut ctx = CommandContext::new();
    ctx.set("path", ArgValue::FilePath("/tmp/test.txt".into()));
    assert_eq!(ctx.string("path"), Some("/tmp/test.txt"));
}

#[test]
fn test_command_context_string_from_motion() {
    let mut ctx = CommandContext::new();
    ctx.set("motion", ArgValue::Motion("word-forward".into()));
    assert_eq!(ctx.string("motion"), Some("word-forward"));
}

#[test]
fn test_command_context_string_wrong_type_returns_none() {
    let mut ctx = CommandContext::new();
    ctx.set("val", ArgValue::Count(5));
    assert!(ctx.string("val").is_none());
}

#[test]
fn test_command_context_string_missing_returns_none() {
    let ctx = CommandContext::new();
    assert!(ctx.string("nonexistent").is_none());
}

#[test]
fn test_command_context_range() {
    let mut ctx = CommandContext::new();
    ctx.set("range", ArgValue::Range(1, 10));
    assert_eq!(ctx.range(), Some((1, 10)));
}

#[test]
fn test_command_context_range_wrong_type_returns_none() {
    let mut ctx = CommandContext::new();
    ctx.set("range", ArgValue::Count(5));
    assert!(ctx.range().is_none());
}

#[test]
fn test_command_context_range_missing_returns_none() {
    let ctx = CommandContext::new();
    assert!(ctx.range().is_none());
}

#[test]
fn test_command_context_buffer_id() {
    let mut ctx = CommandContext::new();
    assert!(ctx.buffer_id().is_none());

    let id = BufferId::from_raw(42);
    ctx.set_buffer_id(id);
    assert_eq!(ctx.buffer_id(), Some(BufferId::from_raw(42)));
}

#[test]
fn test_command_context_buffer_id_zero() {
    let mut ctx = CommandContext::new();
    ctx.set_buffer_id(BufferId::from_raw(0));
    assert_eq!(ctx.buffer_id(), Some(BufferId::from_raw(0)));
}

#[test]
fn test_command_context_buffer_id_wrong_type_returns_none() {
    let mut ctx = CommandContext::new();
    ctx.set("buffer_id", ArgValue::Count(42));
    assert!(ctx.buffer_id().is_none());
}

// === window_id() tests ===

#[test]
fn test_command_context_window_id_none_by_default() {
    let ctx = CommandContext::new();
    assert!(ctx.window_id().is_none());
}

#[test]
fn test_command_context_set_window_id() {
    let mut ctx = CommandContext::new();
    let id = WindowId::from_raw(7);
    ctx.set_window_id(id);
    assert_eq!(ctx.window_id(), Some(WindowId::from_raw(7)));
}

#[test]
fn test_command_context_window_id_zero() {
    let mut ctx = CommandContext::new();
    ctx.set_window_id(WindowId::from_raw(0));
    assert_eq!(ctx.window_id(), Some(WindowId::from_raw(0)));
}

#[test]
fn test_command_context_window_id_wrong_type_returns_none() {
    let mut ctx = CommandContext::new();
    ctx.set("window_id", ArgValue::Count(7));
    assert!(ctx.window_id().is_none());
}

#[test]
fn test_command_context_vfs_none_by_default() {
    let ctx = CommandContext::new();
    assert!(ctx.vfs().is_none());
}

#[test]
fn test_command_context_set_vfs() {
    use reovim_driver_vfs::MockVfs;

    let mut ctx = CommandContext::new();
    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    ctx.set_vfs(vfs);
    assert!(ctx.vfs().is_some());
}

#[test]
fn test_command_context_with_vfs() {
    use reovim_driver_vfs::MockVfs;

    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let ctx = CommandContext::new().with_vfs(vfs);
    assert!(ctx.vfs().is_some());
}

#[test]
fn test_command_context_debug_with_vfs() {
    use reovim_driver_vfs::MockVfs;

    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let ctx = CommandContext::new().with_vfs(vfs);
    let debug_str = format!("{ctx:?}");
    assert!(debug_str.contains("<VfsDriver>"));
}

#[test]
fn test_command_context_debug_without_vfs() {
    let ctx = CommandContext::new();
    let debug_str = format!("{ctx:?}");
    assert!(debug_str.contains("CommandContext"));
    assert!(debug_str.contains("None"));
}

#[test]
fn test_command_context_debug_with_args() {
    let mut ctx = CommandContext::new();
    ctx.set("count", ArgValue::Count(3));
    let debug_str = format!("{ctx:?}");
    assert!(debug_str.contains("args"));
}

#[test]
fn test_command_context_mode_name_none_by_default() {
    let ctx = CommandContext::new();
    assert!(ctx.mode_name().is_none());
}

#[test]
fn test_command_context_set_mode_name() {
    let mut ctx = CommandContext::new();
    ctx.set_mode_name("operator-pending");
    assert_eq!(ctx.mode_name(), Some("operator-pending"));
}

#[test]
fn test_command_context_set_mode_name_from_string() {
    let mut ctx = CommandContext::new();
    ctx.set_mode_name(String::from("visual"));
    assert_eq!(ctx.mode_name(), Some("visual"));
}

#[test]
fn test_command_context_set_mode_name_overwrites() {
    let mut ctx = CommandContext::new();
    ctx.set_mode_name("normal");
    ctx.set_mode_name("insert");
    assert_eq!(ctx.mode_name(), Some("insert"));
}

#[test]
fn test_command_context_is_operator_pending() {
    let mut ctx = CommandContext::new();
    assert!(!ctx.is_operator_pending());

    ctx.set_mode_name("operator-pending");
    assert!(ctx.is_operator_pending());

    ctx.set_mode_name("normal");
    assert!(!ctx.is_operator_pending());
}

#[test]
fn test_command_context_is_operator_pending_no_mode_set() {
    let ctx = CommandContext::new();
    assert!(!ctx.is_operator_pending());
}

#[test]
fn test_command_context_cursor_position_none_by_default() {
    let ctx = CommandContext::new();
    assert!(ctx.cursor_position().is_none());
}

#[test]
fn test_command_context_set_cursor_position() {
    let mut ctx = CommandContext::new();
    let pos = Position::new(10, 5);
    ctx.set_cursor_position(pos);

    let result = ctx.cursor_position();
    assert!(result.is_some());
    let cursor = result.unwrap();
    assert_eq!(cursor.line, 10);
    assert_eq!(cursor.column, 5);
}

#[test]
fn test_command_context_set_cursor_position_origin() {
    let mut ctx = CommandContext::new();
    ctx.set_cursor_position(Position::new(0, 0));
    let pos = ctx.cursor_position().expect("cursor should be set");
    assert_eq!(pos.line, 0);
    assert_eq!(pos.column, 0);
}

#[test]
fn test_command_context_cursor_position_explicit_passing() {
    // Simulating runner pattern: set cursor before command dispatch
    let mut ctx = CommandContext::new();

    // Runner sets cursor from active window
    ctx.set_cursor_position(Position::new(42, 17));

    // Command reads cursor - should get exactly what was set
    let pos = ctx.cursor_position().expect("cursor should be set");
    assert_eq!(pos.line, 42);
    assert_eq!(pos.column, 17);
}

// === char() tests ===

#[test]
fn test_command_context_char() {
    let mut ctx = CommandContext::new();
    ctx.set("target", ArgValue::Char('x'));
    assert_eq!(ctx.char("target"), Some('x'));
}

#[test]
fn test_command_context_char_missing_returns_none() {
    let ctx = CommandContext::new();
    assert!(ctx.char("target").is_none());
}

#[test]
fn test_command_context_char_wrong_type_returns_none() {
    let mut ctx = CommandContext::new();
    ctx.set("target", ArgValue::Register('x'));
    assert!(ctx.char("target").is_none());
}

// === range_start() / range_end() tests ===

#[test]
fn test_command_context_range_start() {
    let mut ctx = CommandContext::new();
    ctx.set("range_start", ArgValue::Position(5, 3));
    assert_eq!(ctx.range_start(), Some((5, 3)));
}

#[test]
fn test_command_context_range_start_missing_returns_none() {
    let ctx = CommandContext::new();
    assert!(ctx.range_start().is_none());
}

#[test]
fn test_command_context_range_start_wrong_type_returns_none() {
    let mut ctx = CommandContext::new();
    ctx.set("range_start", ArgValue::Count(5));
    assert!(ctx.range_start().is_none());
}

#[test]
fn test_command_context_range_end() {
    let mut ctx = CommandContext::new();
    ctx.set("range_end", ArgValue::Position(10, 0));
    assert_eq!(ctx.range_end(), Some((10, 0)));
}

#[test]
fn test_command_context_range_end_missing_returns_none() {
    let ctx = CommandContext::new();
    assert!(ctx.range_end().is_none());
}

#[test]
fn test_command_context_range_end_wrong_type_returns_none() {
    let mut ctx = CommandContext::new();
    ctx.set("range_end", ArgValue::Range(1, 10));
    assert!(ctx.range_end().is_none());
}

// === is_linewise() tests ===

#[test]
fn test_command_context_is_linewise_true() {
    let mut ctx = CommandContext::new();
    ctx.set("linewise", ArgValue::Bool(true));
    assert!(ctx.is_linewise());
}

#[test]
fn test_command_context_is_linewise_false() {
    let mut ctx = CommandContext::new();
    ctx.set("linewise", ArgValue::Bool(false));
    assert!(!ctx.is_linewise());
}

#[test]
fn test_command_context_is_linewise_missing() {
    let ctx = CommandContext::new();
    assert!(!ctx.is_linewise());
}

#[test]
fn test_command_context_is_linewise_wrong_type() {
    let mut ctx = CommandContext::new();
    ctx.set("linewise", ArgValue::Count(1));
    assert!(!ctx.is_linewise());
}

// === bool_flag() tests ===

#[test]
fn test_command_context_bool_flag_true() {
    let mut ctx = CommandContext::new();
    ctx.set("inclusive", ArgValue::Bool(true));
    assert!(ctx.bool_flag("inclusive"));
}

#[test]
fn test_command_context_bool_flag_false() {
    let mut ctx = CommandContext::new();
    ctx.set("inclusive", ArgValue::Bool(false));
    assert!(!ctx.bool_flag("inclusive"));
}

#[test]
fn test_command_context_bool_flag_missing() {
    let ctx = CommandContext::new();
    assert!(!ctx.bool_flag("inclusive"));
}

#[test]
fn test_command_context_bool_flag_wrong_type() {
    let mut ctx = CommandContext::new();
    ctx.set("inclusive", ArgValue::Bang(true));
    assert!(!ctx.bool_flag("inclusive"));
}

// === get() tests ===

#[test]
fn test_command_context_get() {
    let mut ctx = CommandContext::new();
    ctx.set("count", ArgValue::Count(5));
    let val = ctx.get("count");
    assert!(val.is_some());
    assert_eq!(val.unwrap(), &ArgValue::Count(5));
}

#[test]
fn test_command_context_get_missing() {
    let ctx = CommandContext::new();
    assert!(ctx.get("nonexistent").is_none());
}

// === set() overwrite tests ===

#[test]
fn test_command_context_set_overwrites() {
    let mut ctx = CommandContext::new();
    ctx.set("count", ArgValue::Count(1));
    ctx.set("count", ArgValue::Count(99));
    assert_eq!(ctx.count(), Some(99));
}

// === Multiple args coexistence ===

#[test]
fn test_command_context_multiple_args() {
    let mut ctx = CommandContext::new();
    ctx.set("count", ArgValue::Count(3));
    ctx.set("register", ArgValue::Register('a'));
    ctx.set("bang", ArgValue::Bang(true));
    ctx.set("file", ArgValue::FilePath("test.txt".into()));
    ctx.set("range", ArgValue::Range(1, 10));
    ctx.set_buffer_id(BufferId::from_raw(7));
    ctx.set_mode_name("normal");
    ctx.set_cursor_position(Position::new(5, 2));

    assert_eq!(ctx.count(), Some(3));
    assert_eq!(ctx.register(), Some('a'));
    assert!(ctx.has_bang());
    assert_eq!(ctx.string("file"), Some("test.txt"));
    assert_eq!(ctx.range(), Some((1, 10)));
    assert_eq!(ctx.buffer_id(), Some(BufferId::from_raw(7)));
    assert_eq!(ctx.mode_name(), Some("normal"));
    let pos = ctx.cursor_position().unwrap();
    assert_eq!(pos.line, 5);
    assert_eq!(pos.column, 2);
}

// === Clone tests ===

#[test]
fn test_command_context_clone() {
    let mut ctx = CommandContext::new();
    ctx.set("count", ArgValue::Count(5));
    ctx.set_mode_name("normal");

    let cloned = ctx.clone();
    assert_eq!(cloned.count(), Some(5));
    assert_eq!(cloned.mode_name(), Some("normal"));
}

// =========================================================================
// #714 repro: Box::leak at input.rs:946 is unnecessary.
// CommandContext::set() takes &str, not &'static str.
// The Box::leak exists because the caller does Box::leak(key.into_boxed_str())
// when it could just pass &key.
// =========================================================================

#[test]
fn b3_repro_set_accepts_borrowed_str() {
    let mut ctx = CommandContext::new();

    // set() takes &str — no need for &'static str or Box::leak
    let key = String::from("count");
    ctx.set(&key, ArgValue::Count(42));
    assert_eq!(ctx.count(), Some(42));

    // This proves input.rs:946 could just do:
    //   cmd_ctx.set(&key, value);
    // Instead of:
    //   cmd_ctx.set(Box::leak(key.into_boxed_str()), value);
    //
    // The Box::leak is gratuitous — set() calls name.to_owned() internally,
    // so it makes its own copy regardless. The leak wastes memory.
}
