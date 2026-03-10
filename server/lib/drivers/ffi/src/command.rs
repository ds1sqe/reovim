//! Command execution and registration FFI functions.
//!
//! This module provides:
//! - **Execution**: FFI modules can invoke registered commands
//! - **Registration**: FFI modules can register command handlers that the
//!   engine calls back into during command execution
//!
//! # Execution (5a)
//!
//! `reovim_execute_command()` parses a `"module:command"` string, builds a
//! `CommandContext`, and delegates to `CommandApi::execute_command()`.
//!
//! # Registration (5b)
//!
//! `reovim_register_command()` creates an `FfiCommandHandler` that wraps a
//! C function pointer. During command execution, the engine calls the
//! handler's `execute()` method, which sets up the `RuntimeGuard` and
//! invokes the C callback.

#![allow(unsafe_code)]

use std::ffi::CStr;

use {
    libc::c_char,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::api::CommandApi,
    reovim_kernel::api::v1::CommandId,
};

#[allow(clippy::wildcard_imports)]
use crate::error::*;
use std::panic::AssertUnwindSafe;

use crate::{
    ffi_types::{ReovimCommandArgs, ReovimPosition},
    runtime::{ffi_catch_unwind, with_runtime},
};

// ============================================================================
// Command result constants (positive values for non-error results)
// ============================================================================

/// Command returned `CommandResult::Success`.
pub const REOVIM_CMD_SUCCESS: i32 = 0;

/// Command signaled `RuntimeSignal::Quit` (via signal queue).
pub const REOVIM_CMD_QUIT: i32 = 1;

/// Command returned `CommandResult::Error(...)`.
pub const REOVIM_CMD_ERROR: i32 = 4;

// ============================================================================
// 5a: Execute commands from FFI
// ============================================================================

/// Execute a command by ID string.
///
/// The `cmd_id` string must be in `"module:command"` format. Optional count
/// and register can be passed; use `count < 0` to indicate no count, and
/// register `0` to indicate no register.
///
/// # Returns
///
/// - `REOVIM_CMD_SUCCESS` (0) on success
/// - `REOVIM_CMD_QUIT` (1) if command signaled quit (via `RuntimeSignal`)
/// - `REOVIM_CMD_ERROR` (4) if command returned an error
/// - `REOVIM_ERR_NULL_PTR` if `cmd_id` is null
/// - `REOVIM_ERR_INVALID_UTF8` if `cmd_id` is not valid UTF-8
/// - `REOVIM_ERR_NO_RUNTIME` if called outside a command callback
///
/// # Safety
///
/// - `cmd_id` must be a valid null-terminated C string or null
/// - Must be called during a command callback (`RuntimeGuard` active)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_execute_command(
    cmd_id: *const c_char,
    count: i32,
    register: u8,
) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if cmd_id.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let c_str = unsafe { CStr::from_ptr(cmd_id) };
        let Ok(cmd_str) = c_str.to_str() else {
            return REOVIM_ERR_INVALID_UTF8;
        };

        let command_id = CommandId::from_qualified_leaked(cmd_str.to_string());

        let mut ctx = CommandContext::new();
        if count >= 0 {
            #[allow(clippy::cast_sign_loss)]
            ctx.set("count", reovim_driver_command_types::ArgValue::Count(count as usize));
        }
        if register != 0 {
            ctx.set("register", reovim_driver_command_types::ArgValue::Register(register as char));
        }

        match with_runtime(|rt| {
            let result = rt.execute_command(command_id.clone(), ctx);
            let signals = rt.take_signals();
            (result, signals)
        }) {
            Err(e) => e,
            Ok((result, signals)) => {
                if signals
                    .iter()
                    .any(|s| matches!(s, reovim_driver_command_types::RuntimeSignal::Quit))
                {
                    REOVIM_CMD_QUIT
                } else {
                    match result {
                        CommandResult::Success => REOVIM_CMD_SUCCESS,
                        CommandResult::Error(_) => REOVIM_CMD_ERROR,
                    }
                }
            }
        }
    }))
}

// ============================================================================
// 5b: Command callback types
// ============================================================================

/// C function pointer type for command callbacks.
///
/// Called by the engine when the registered command is executed.
///
/// # Parameters
///
/// - `user_data`: Opaque pointer passed during registration
/// - `args`: Parsed command arguments (count, register, cursor)
///
/// # Returns
///
/// `REOVIM_CMD_SUCCESS` (0) for success, or a positive result code.
pub type ReovimCommandCallback = Option<
    unsafe extern "C" fn(user_data: *mut libc::c_void, args: *const ReovimCommandArgs) -> i32,
>;

/// Command registration info passed to `reovim_register_command`.
#[repr(C)]
pub struct ReovimCommandRegistration {
    /// Command ID in "module:command" format (null-terminated).
    pub id: *const c_char,
    /// Human-readable description (null-terminated).
    pub description: *const c_char,
    /// Callback function.
    pub callback: ReovimCommandCallback,
    /// Opaque user data passed to callback.
    pub user_data: *mut libc::c_void,
}

// ============================================================================
// 5b: FfiCommandHandler
// ============================================================================

/// A command handler that bridges to an FFI callback function.
///
/// Created by `reovim_register_command()` during module init. When the engine
/// executes this command, it calls the FFI callback with a `RuntimeGuard`
/// active so the callback can use buffer/window/mode APIs.
pub struct FfiCommandHandler {
    /// Command identifier.
    id: CommandId,
    /// Human-readable description.
    // SAFETY: intentional leak — command descriptions live for program lifetime.
    // Same pattern as ffi-python/src/types.rs (PyCommandRegistration::to_kernel).
    description: &'static str,
    /// C callback function pointer.
    callback: unsafe extern "C" fn(*mut libc::c_void, *const ReovimCommandArgs) -> i32,
    /// Opaque user data for the callback.
    user_data: *mut libc::c_void,
}

// Safety: user_data is an opaque pointer owned by the external module.
// The module guarantees it remains valid for the handler's lifetime.
unsafe impl Send for FfiCommandHandler {}
unsafe impl Sync for FfiCommandHandler {}

impl FfiCommandHandler {
    /// Get the command ID.
    #[must_use]
    pub const fn id(&self) -> &CommandId {
        &self.id
    }

    /// Get the command description.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        self.description
    }

    /// Execute the FFI callback with the given command context.
    ///
    /// Converts `CommandContext` to `ReovimCommandArgs` and calls the C callback.
    #[must_use]
    pub fn execute_callback(&self, ctx: &CommandContext) -> i32 {
        let args = command_context_to_ffi_args(ctx);
        // Safety: caller ensures user_data is valid and callback is safe to call.
        unsafe { (self.callback)(self.user_data, &raw const args) }
    }
}

/// Convert a `CommandContext` to `ReovimCommandArgs` for the FFI callback.
#[allow(clippy::cast_possible_truncation)]
fn command_context_to_ffi_args(ctx: &CommandContext) -> ReovimCommandArgs {
    let (has_count, count) = ctx.count().map_or((0, 0), |n| (1, n as u32));

    let (has_register, register) = ctx.register().map_or((0, 0), |c| (1, c as u8));

    let (has_cursor, cursor) = ctx
        .cursor_position()
        .map_or_else(|| (0, ReovimPosition::new(0, 0)), |pos| (1, ReovimPosition::from(pos)));

    ReovimCommandArgs {
        has_count,
        count,
        has_register,
        register,
        pad: [0; 3],
        has_cursor,
        cursor,
    }
}

// ============================================================================
// 5c: Command registration via InitGuard
// ============================================================================

/// Register a command handler from an FFI module.
///
/// Must be called during module init (when [`InitGuard`] is active).
/// The handler is stored in the `FfiCommandHandlerStore` service.
///
/// # Returns
///
/// - `REOVIM_OK` on success
/// - `REOVIM_ERR_NULL_PTR` if `reg` is null or has null fields
/// - `REOVIM_ERR_INVALID_UTF8` if strings are not valid UTF-8
/// - `REOVIM_ERR_NO_INIT_CTX` if called outside module init
///
/// # Safety
///
/// - `reg` must be a valid pointer to a `ReovimCommandRegistration`
/// - All string fields in `reg` must be valid null-terminated C strings
/// - `callback` must be a valid function pointer
/// - `user_data` must remain valid for the lifetime of the command handler
/// - Must be called during module init (`InitGuard` active)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn reovim_register_command(reg: *const ReovimCommandRegistration) -> i32 {
    ffi_catch_unwind(AssertUnwindSafe(|| {
        if reg.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let reg = unsafe { &*reg };

        if reg.id.is_null() || reg.description.is_null() {
            return REOVIM_ERR_NULL_PTR;
        }

        let Some(callback) = reg.callback else {
            return REOVIM_ERR_NULL_PTR;
        };

        let Ok(id_str) = (unsafe { CStr::from_ptr(reg.id) }).to_str() else {
            return REOVIM_ERR_INVALID_UTF8;
        };

        let Ok(desc_str) = (unsafe { CStr::from_ptr(reg.description) }).to_str() else {
            return REOVIM_ERR_INVALID_UTF8;
        };

        let command_id = CommandId::from_qualified_leaked(id_str.to_string());
        // SAFETY: intentional leak — description lives for program lifetime.
        let description: &'static str = Box::leak(desc_str.to_string().into_boxed_str());

        let handler = FfiCommandHandler {
            id: command_id,
            description,
            callback,
            user_data: reg.user_data,
        };

        crate::runtime::with_services(|services| {
            services
                .get_or_create::<FfiCommandHandlerStore>()
                .add(handler);
        })
        .map_or_else(|e| e, |()| REOVIM_OK)
    }))
}

// ============================================================================
// FfiCommandHandlerStore - Service for storing registered handlers
// ============================================================================

/// Storage for FFI command handlers registered during module init.
///
/// Implements `Service` so it can live in `ServiceRegistry`.
/// After init, the loader retrieves handlers via `take_handlers()`.
#[derive(Default)]
pub struct FfiCommandHandlerStore {
    handlers: std::sync::Mutex<Vec<FfiCommandHandler>>,
}

impl FfiCommandHandlerStore {
    /// Add a handler to the store.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn add(&self, handler: FfiCommandHandler) {
        self.handlers.lock().expect("lock poisoned").push(handler);
    }

    /// Take all handlers out of the store.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn take_handlers(&self) -> Vec<FfiCommandHandler> {
        std::mem::take(&mut *self.handlers.lock().expect("lock poisoned"))
    }

    /// Number of handlers currently stored.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn count(&self) -> usize {
        self.handlers.lock().expect("lock poisoned").len()
    }
}

impl reovim_kernel::api::v1::Service for FfiCommandHandlerStore {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
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
        ctx.set_cursor_position(reovim_kernel::api::v1::Position::new(10, 5));
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
        ctx.set_cursor_position(reovim_kernel::api::v1::Position::new(42, 7));
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
            id: CommandId::from_qualified_leaked("test:add".to_string()),
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
            id: CommandId::from_qualified_leaked("test:a".to_string()),
            description: "a",
            callback: noop_cb,
            user_data: std::ptr::null_mut(),
        });
        store.add(FfiCommandHandler {
            id: CommandId::from_qualified_leaked("test:b".to_string()),
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
}
