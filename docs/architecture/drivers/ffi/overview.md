# ffi/ - Foreign Function Interface Driver

C-compatible interfaces for external module development.

## Source Location

`ext/server/drivers/ffi/src/`

## Purpose

Provides C-compatible interfaces for external module development. Generates a C
header file (`reovim.h`) and provides FFI-safe service wrappers that external
modules can use to interact with the kernel.

Following Linux kernel design (mechanism vs policy):
- **Kernel provides mechanism**: `#[repr(C)]` types, FFI-safe Module trait
- **This driver provides policy**: C header generation, FFI wrappers, ABI versioning

## Architecture

```
+-------------------------------------------------------------+
|             External Modules (C, Haskell, etc.)              |
|   #include <reovim.h>                                        |
|   reovim_log_info("Module loaded");                          |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                 Driver (drivers/ffi/)                        |
|   reovim.h | reovim_log_* | reovim_abi_version               |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                     Kernel (api/v1)                          |
|   Version | ModuleProbe | Module trait | tracing             |
+-------------------------------------------------------------+
```

## Module Entry Points

External modules must export these symbols (matching `declare_module!` output):

| Symbol | Type | Purpose |
|--------|------|---------|
| `REOVIM_MODULE_API_VERSION` | `Version` | Pre-load version check |
| `reovim_module_probe()` | `fn() -> ModuleProbe` | Metadata query |
| `reovim_module_entry()` | `fn() -> *mut c_void` | Instance creation |
| `reovim_module_init()` | `fn(*mut c_void, *const c_void) -> i32` | Init trampoline |
| `reovim_module_exit()` | `fn(*mut c_void) -> i32` | Exit trampoline |
| `reovim_module_destroy()` | `fn(*mut c_void)` | Cleanup trampoline |

## Key Exports

### Logging Functions

```rust
pub extern "C" fn reovim_log_info(msg: *const c_char);
pub extern "C" fn reovim_log_warn(msg: *const c_char);
pub extern "C" fn reovim_log_error(msg: *const c_char);
pub extern "C" fn reovim_log_debug(msg: *const c_char);
```

### Version Checking

```rust
pub const ABI_VERSION: Version;
pub extern "C" fn reovim_abi_version() -> Version;
pub extern "C" fn reovim_abi_is_compatible(version: Version) -> bool;
```

### Timer Wheel

```rust
pub type ReovimTimerCallback = extern "C" fn(*mut c_void);
pub type ReovimTimerHandle = u64;

pub extern "C" fn reovim_schedule_delayed(
    delay_ms: u64,
    callback: ReovimTimerCallback,
    user_data: *mut c_void,
) -> ReovimTimerHandle;

pub extern "C" fn reovim_schedule_periodic(
    interval_ms: u64,
    callback: ReovimTimerCallback,
    user_data: *mut c_void,
) -> ReovimTimerHandle;

pub extern "C" fn reovim_cancel_timer(handle: ReovimTimerHandle) -> bool;
```

## ABI vs API Versioning

The ABI version is separate from the API version:

- **ABI version**: Binary compatibility (struct layouts, calling conventions)
- **API version**: Semantic compatibility (trait methods, behavior)

ABI version bumps when:
- Struct sizes change (e.g., `ModuleProbe` 216 -> 1308 bytes)
- Struct field layouts change
- Function signatures change

API version bumps when:
- New methods are added to traits
- Method semantics change
- New types are added (backwards compatible)

## Generated C Header

The `build.rs` generates `include/reovim.h` with:
- ABI version constants (`REOVIM_ABI_VERSION_*`)
- API version constants (`REOVIM_API_VERSION_*`)
- Type definitions (`ReovimVersion`, `ReovimModuleProbe`)
- Service function declarations

## Related Documents

- [Driver Overview](../overview.md)
- [ffi-python Driver](../ffi-python/overview.md)
- [Module System](../../modules/overview.md)
