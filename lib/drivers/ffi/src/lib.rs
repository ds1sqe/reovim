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

mod logging;
mod types;
mod version;

// Re-export FFI service functions
pub use {
    logging::{reovim_log_debug, reovim_log_error, reovim_log_info, reovim_log_warn},
    version::{ABI_VERSION, reovim_abi_is_compatible, reovim_abi_version},
};

// Re-export kernel types for convenience
pub use reovim_kernel::api::v1::{API_VERSION, ModuleProbe, Version};
