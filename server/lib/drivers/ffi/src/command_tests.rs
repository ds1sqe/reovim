use super::*;

// ========================================================================
// Execute command tests
// ========================================================================

#[test]
fn test_execute_command_no_runtime() {
    let cmd = std::ffi::CString::new("test:noop").unwrap();
    let result = unsafe { reovim_execute_command(cmd.as_ptr(), -1, 0) };
    assert_eq!(result, REOVIM_ERR_NO_RUNTIME);
}

#[test]
fn test_execute_command_null_id() {
    let result = unsafe { reovim_execute_command(std::ptr::null(), -1, 0) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

#[test]
fn test_execute_command_invalid_utf8() {
    let invalid_bytes: &[u8] = &[0x80, 0x00];
    let result = unsafe { reovim_execute_command(invalid_bytes.as_ptr().cast(), -1, 0) };
    assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
}

// ========================================================================
// Command result constants tests
// ========================================================================

#[test]
fn test_command_result_constants() {
    assert_eq!(REOVIM_CMD_SUCCESS, 0);
    const { assert!(REOVIM_CMD_QUIT > 0) };
    const { assert!(REOVIM_CMD_ERROR > 0) };
    // All distinct
    let values = [REOVIM_CMD_SUCCESS, REOVIM_CMD_QUIT, REOVIM_CMD_ERROR];
    for (i, &a) in values.iter().enumerate() {
        for (j, &b) in values.iter().enumerate() {
            if i != j {
                assert_ne!(a, b);
            }
        }
    }
}

// ========================================================================
// command_context_to_ffi_args tests
// ========================================================================

#[test]
fn test_context_to_ffi_args_empty() {
    let ctx = CommandContext::new();
    let args = command_context_to_ffi_args(&ctx);
    assert_eq!(args.has_count, 0);
    assert_eq!(args.has_register, 0);
    assert_eq!(args.has_cursor, 0);
}

#[test]
fn test_context_to_ffi_args_with_count() {
    let mut ctx = CommandContext::new();
    ctx.set("count", reovim_driver_command_types::ArgValue::Count(5));
    let args = command_context_to_ffi_args(&ctx);
    assert_eq!(args.has_count, 1);
    assert_eq!(args.count, 5);
}

#[test]
fn test_context_to_ffi_args_with_register() {
    let mut ctx = CommandContext::new();
    ctx.set("register", reovim_driver_command_types::ArgValue::Register('a'));
    let args = command_context_to_ffi_args(&ctx);
    assert_eq!(args.has_register, 1);
    assert_eq!(args.register, b'a');
}

#[test]
fn test_context_to_ffi_args_with_cursor() {
    let mut ctx = CommandContext::new();
    ctx.set_cursor_position(reovim_domain_text::Position::new(10, 5));
    let args = command_context_to_ffi_args(&ctx);
    assert_eq!(args.has_cursor, 1);
    assert_eq!(args.cursor.line, 10);
    assert_eq!(args.cursor.column, 5);
}

#[test]
fn test_context_to_ffi_args_fully_populated() {
    let mut ctx = CommandContext::new();
    ctx.set("count", reovim_driver_command_types::ArgValue::Count(3));
    ctx.set("register", reovim_driver_command_types::ArgValue::Register('"'));
    ctx.set_cursor_position(reovim_domain_text::Position::new(42, 7));
    let args = command_context_to_ffi_args(&ctx);
    assert_eq!(args.has_count, 1);
    assert_eq!(args.count, 3);
    assert_eq!(args.has_register, 1);
    assert_eq!(args.register, b'"');
    assert_eq!(args.has_cursor, 1);
    assert_eq!(args.cursor, ReovimPosition::new(42, 7));
}

// ========================================================================
// Register command tests
// ========================================================================

#[test]
fn test_register_command_null_reg() {
    let result = unsafe { reovim_register_command(std::ptr::null()) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

unsafe extern "C" fn dummy_cb(_: *mut libc::c_void, _: *const ReovimCommandArgs) -> i32 {
    0
}

unsafe extern "C" fn hello_cb(_: *mut libc::c_void, _: *const ReovimCommandArgs) -> i32 {
    0
}

#[test]
fn test_register_command_null_id() {
    let desc = std::ffi::CString::new("test").unwrap();
    let reg = ReovimCommandRegistration {
        id: std::ptr::null(),
        description: desc.as_ptr(),
        callback: Some(dummy_cb),
        user_data: std::ptr::null_mut(),
    };
    let result = unsafe { reovim_register_command(&raw const reg) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

#[test]
fn test_register_command_null_description() {
    let id = std::ffi::CString::new("test:cmd").unwrap();
    let reg = ReovimCommandRegistration {
        id: id.as_ptr(),
        description: std::ptr::null(),
        callback: Some(dummy_cb),
        user_data: std::ptr::null_mut(),
    };
    let result = unsafe { reovim_register_command(&raw const reg) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

#[test]
fn test_register_command_null_callback() {
    let id = std::ffi::CString::new("test:cmd").unwrap();
    let desc = std::ffi::CString::new("test command").unwrap();
    let reg = ReovimCommandRegistration {
        id: id.as_ptr(),
        description: desc.as_ptr(),
        callback: None,
        user_data: std::ptr::null_mut(),
    };
    let result = unsafe { reovim_register_command(&raw const reg) };
    assert_eq!(result, REOVIM_ERR_NULL_PTR);
}

#[test]
fn test_register_command_no_init_ctx() {
    let id = std::ffi::CString::new("test:cmd").unwrap();
    let desc = std::ffi::CString::new("test command").unwrap();
    let reg = ReovimCommandRegistration {
        id: id.as_ptr(),
        description: desc.as_ptr(),
        callback: Some(dummy_cb),
        user_data: std::ptr::null_mut(),
    };
    let result = unsafe { reovim_register_command(&raw const reg) };
    assert_eq!(result, REOVIM_ERR_NO_INIT_CTX);
}

#[test]
fn test_register_command_with_init_guard() {
    use reovim_kernel::api::v1::ServiceRegistry;

    let registry = ServiceRegistry::new();
    let _guard = unsafe { crate::runtime::InitGuard::new(&registry) };

    let id = std::ffi::CString::new("test:hello").unwrap();
    let desc = std::ffi::CString::new("say hello").unwrap();

    let reg = ReovimCommandRegistration {
        id: id.as_ptr(),
        description: desc.as_ptr(),
        callback: Some(hello_cb),
        user_data: std::ptr::null_mut(),
    };

    let result = unsafe { reovim_register_command(&raw const reg) };
    assert_eq!(result, REOVIM_OK);

    // Verify the handler was stored
    let store = registry
        .get::<FfiCommandHandlerStore>()
        .expect("store should exist");
    assert_eq!(store.count(), 1);

    let handlers = store.take_handlers();
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].id().module().as_str(), "test");
    assert_eq!(handlers[0].id().name(), "hello");
    assert_eq!(handlers[0].description(), "say hello");
}

// ========================================================================
// FfiCommandHandler tests
// ========================================================================

unsafe extern "C" fn add_callback(
    user_data: *mut libc::c_void,
    args: *const ReovimCommandArgs,
) -> i32 {
    let data = unsafe { &*(user_data.cast::<i32>()) };
    let args = unsafe { &*args };
    #[allow(clippy::cast_possible_wrap)]
    let count = args.count as i32;
    *data + count
}

#[test]
fn test_ffi_command_handler_execute_callback() {
    let mut data: i32 = 10;
    let handler = FfiCommandHandler {
        id: CommandId::from_qualified("test:add".to_string()),
        description: "adds",
        callback: add_callback,
        user_data: (&raw mut data).cast(),
    };

    let mut ctx = CommandContext::new();
    ctx.set("count", reovim_driver_command_types::ArgValue::Count(5));

    let result = handler.execute_callback(&ctx);
    assert_eq!(result, 15); // 10 + 5
}

// ========================================================================
// FfiCommandHandlerStore tests
// ========================================================================

#[test]
fn test_handler_store_empty() {
    let store = FfiCommandHandlerStore::default();
    assert_eq!(store.count(), 0);
    assert!(store.take_handlers().is_empty());
}

unsafe extern "C" fn noop_cb(_: *mut libc::c_void, _: *const ReovimCommandArgs) -> i32 {
    0
}

#[test]
fn test_handler_store_add_and_take() {
    let store = FfiCommandHandlerStore::default();
    store.add(FfiCommandHandler {
        id: CommandId::from_qualified("test:a".to_string()),
        description: "a",
        callback: noop_cb,
        user_data: std::ptr::null_mut(),
    });
    store.add(FfiCommandHandler {
        id: CommandId::from_qualified("test:b".to_string()),
        description: "b",
        callback: noop_cb,
        user_data: std::ptr::null_mut(),
    });

    assert_eq!(store.count(), 2);

    let handlers = store.take_handlers();
    assert_eq!(handlers.len(), 2);
    assert_eq!(store.count(), 0); // drained
}

#[test]
fn test_register_command_invalid_utf8_id() {
    let desc = std::ffi::CString::new("test").unwrap();
    let invalid_bytes: &[u8] = &[0x80, 0x00];
    let reg = ReovimCommandRegistration {
        id: invalid_bytes.as_ptr().cast(),
        description: desc.as_ptr(),
        callback: Some(dummy_cb),
        user_data: std::ptr::null_mut(),
    };
    let result = unsafe { reovim_register_command(&raw const reg) };
    assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
}

#[test]
fn test_register_command_invalid_utf8_desc() {
    let id = std::ffi::CString::new("test:cmd").unwrap();
    let invalid_bytes: &[u8] = &[0x80, 0x00];
    let reg = ReovimCommandRegistration {
        id: id.as_ptr(),
        description: invalid_bytes.as_ptr().cast(),
        callback: Some(dummy_cb),
        user_data: std::ptr::null_mut(),
    };
    let result = unsafe { reovim_register_command(&raw const reg) };
    assert_eq!(result, REOVIM_ERR_INVALID_UTF8);
}

// ========================================================================
// With-runtime tests (RuntimeGuard + TestSessionRuntime)
// ========================================================================

#[test]
fn test_execute_command_not_found_with_runtime() {
    use {crate::runtime::RuntimeGuard, reovim_driver_session::testing::TestSessionRuntime};

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let mut rt = harness.runtime();
    let _guard = unsafe { RuntimeGuard::new(&mut rt) };

    let cmd = std::ffi::CString::new("nonexistent:command").unwrap();
    let result = unsafe { reovim_execute_command(cmd.as_ptr(), -1, 0) };
    // Command not found → CommandResult::Error
    assert_eq!(result, REOVIM_CMD_ERROR);
}
