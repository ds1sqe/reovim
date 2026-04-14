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
    reovim_driver_text_session::api::CommandApi,
    reovim_kernel::api::v1::CommandId,
    reovim_subsys_command_types::{CommandContext, CommandResult},
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
#[cfg_attr(coverage_nightly, coverage(off))]
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

        let command_id = CommandId::from_qualified(cmd_str.to_string());

        let mut ctx = CommandContext::new();
        if count >= 0 {
            #[allow(clippy::cast_sign_loss)]
            ctx.set("count", reovim_subsys_command_types::ArgValue::Count(count as usize));
        }
        if register != 0 {
            ctx.set("register", reovim_subsys_command_types::ArgValue::Register(register as char));
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
                    .any(|s| matches!(s, reovim_subsys_command_types::RuntimeSignal::Quit))
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

    let (has_cursor, cursor) = ctx.cursor_position().map_or_else(
        || (0, ReovimPosition::new(0, 0)),
        |(line, col)| (1, ReovimPosition::new(line as u32, col as u32)),
    );

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
#[cfg_attr(coverage_nightly, coverage(off))]
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

        let command_id = CommandId::from_qualified(id_str.to_string());
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
#[path = "command_tests.rs"]
mod tests;
