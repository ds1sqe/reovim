#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! FFI driver for reovim.
//!
//! This crate inherently requires unsafe code for FFI operations.
#![allow(unsafe_code)]
//!
//! This driver provides C-compatible interfaces for external module development.
//! It generates a C header file (`reovim.h`) and provides FFI-safe service wrappers
//! that external modules can use to interact with the kernel.
//!
//! # Architecture
//!
//! Following Linux kernel design (mechanism vs policy):
//! - **Kernel provides mechanism**: `#[repr(C)]` types, FFI-safe Module trait
//! - **This driver provides policy**: C header generation, FFI wrappers, ABI versioning
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │             External Modules (C, Haskell, etc.)                  │
//! │   #include <reovim.h>                                            │
//! │   reovim_log_info("Module loaded");                              │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                 Driver (drivers/ffi/)                            │
//! │   reovim.h │ reovim_log_* │ reovim_abi_version                   │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Kernel (api/v1)                              │
//! │   Version │ ModuleProbe │ Module trait │ tracing                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! External modules written in C (or any language with C FFI) can:
//!
//! 1. Include the generated `reovim.h` header
//! 2. Implement the required module entry points
//! 3. Use service wrappers for logging and version checks
//!
//! # Generated C Header
//!
//! The `build.rs` generates `include/reovim.h` with:
//! - ABI version constants (`REOVIM_ABI_VERSION_*`)
//! - API version constants (`REOVIM_API_VERSION_*`)
//! - Type definitions (`ReovimVersion`, `ReovimModuleProbe`)
//! - Service function declarations (`reovim_log_*`, `reovim_abi_is_compatible`)
//!
//! # Module Entry Points
//!
//! External modules must export these symbols (matching `declare_module!` output):
//!
//! | Symbol | Type | Purpose |
//! |--------|------|---------|
//! | `REOVIM_MODULE_API_VERSION` | `Version` | Pre-load version check |
//! | `reovim_module_probe()` | `fn() -> ModuleProbe` | Metadata query |
//! | `reovim_module_entry()` | `fn() -> *mut c_void` | Instance creation |
//! | `reovim_module_init()` | `fn(*mut c_void, *const c_void) -> i32` | Init trampoline |
//! | `reovim_module_exit()` | `fn(*mut c_void) -> i32` | Exit trampoline |
//! | `reovim_module_destroy()` | `fn(*mut c_void)` | Cleanup trampoline |
//!
//! # ABI Versioning
//!
//! The ABI version is separate from the API version:
//! - **ABI version**: Binary compatibility (struct layouts, calling conventions)
//! - **API version**: Semantic compatibility (trait methods, behavior)
//!
//! ABI version bumps when:
//! - Struct sizes change (e.g., `ModuleProbe` 216 → 1308 bytes)
//! - Struct field layouts change
//! - Function signatures change
//!
//! API version bumps when:
//! - New methods are added to traits
//! - Method semantics change
//! - New types are added (backwards compatible)

pub mod buffer;
pub mod command;
pub mod error;
pub mod event;
pub mod ffi_types;
mod logging;
pub mod mode;
pub mod runtime;
mod timer;
mod types;
mod version;
pub mod window;

// Re-export FFI service functions
pub use {
    logging::{reovim_log_debug, reovim_log_error, reovim_log_info, reovim_log_warn},
    timer::{
        ReovimTimerCallback, ReovimTimerHandle, init_timer_wheel, reovim_cancel_timer,
        reovim_schedule_delayed, reovim_schedule_periodic, reovim_timer_count,
        reovim_timer_is_pending,
    },
    version::{ABI_VERSION, reovim_abi_is_compatible, reovim_abi_version},
};

// Re-export runtime bridge types
pub use runtime::{InitGuard, RuntimeGuard, with_runtime, with_services};

// Re-export FFI types
pub use ffi_types::{ReovimCommandArgs, ReovimPosition, ReovimStringResult};

// Re-export buffer API
pub use buffer::{
    ReovimBufferId, reovim_active_buffer, reovim_buffer_content, reovim_buffer_line,
    reovim_buffer_line_count, reovim_buffer_line_len, reovim_buffer_text_range,
    reovim_create_buffer, reovim_delete_buffer, reovim_delete_range, reovim_insert_text,
};

// Re-export window API
pub use window::{
    ReovimWindowId, reovim_active_window, reovim_close_window, reovim_create_window,
    reovim_cursor_position, reovim_focus_window, reovim_window_buffer, reovim_window_count,
};

// Re-export mode API
pub use mode::{
    reovim_current_mode, reovim_mode_depth, reovim_pop_mode, reovim_push_mode, reovim_set_mode,
};

// Re-export command API
pub use command::{
    FfiCommandHandler, FfiCommandHandlerStore, ReovimCommandCallback, ReovimCommandRegistration,
    reovim_execute_command, reovim_register_command,
};

// Re-export event API
pub use event::{
    FfiEventSubscriptionStore, ReovimEventCallback, ReovimSubscriptionHandle,
    reovim_subscribe_event, reovim_unsubscribe_event,
};

// Re-export error codes
pub use error::*;

// Re-export kernel types for convenience
pub use reovim_kernel::api::v1::{API_VERSION, ModuleProbe, Version};
