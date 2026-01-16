# FFI Module Interface

This document describes the Foreign Function Interface (FFI) for external modules
that wish to extend reovim. Modules can be written in C, Haskell, or any language
with C FFI capabilities.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│             External Modules (C, Haskell, etc.)                  │
│   #include <reovim.h>                                            │
│   Implements: reovim_module_*() entry points                     │
├─────────────────────────────────────────────────────────────────┤
│                   FFI Driver (drivers/ffi/)                      │
│   reovim.h │ reovim_log_* │ reovim_abi_version                   │
├─────────────────────────────────────────────────────────────────┤
│                     Kernel (api/v1)                              │
│   Version │ ModuleProbe │ Module trait                           │
└─────────────────────────────────────────────────────────────────┘
```

## Versioning

### ABI Version (1.0.0)

The ABI version tracks **binary compatibility**:
- Struct layouts and sizes
- Function signatures and calling conventions
- Symbol names

An ABI version bump means all external modules must be recompiled.

### API Version (0.2.0)

The API version tracks **semantic compatibility**:
- Module trait methods and behavior
- Lifecycle semantics
- New types and functionality

### Compatibility Rules

```
Required Version    Provided Version    Compatible?
1.0.0               1.0.0               Yes (exact match)
1.0.0               1.1.0               Yes (higher minor)
1.0.0               1.0.5               Yes (patch ignored)
1.2.0               1.1.0               NO  (minor too new)
1.0.0               2.0.0               NO  (major mismatch)
2.0.0               1.0.0               NO  (major mismatch)
```

## Module Entry Points

External modules must export these symbols:

| Symbol | Signature | Purpose |
|--------|-----------|---------|
| `REOVIM_MODULE_API_VERSION` | `const ReovimVersion` | Pre-load version check |
| `reovim_module_probe` | `() -> ReovimModuleProbe` | Metadata query |
| `reovim_module_entry` | `() -> *mut void` | Instance creation |
| `reovim_module_init` | `(*mut void, *const void) -> i32` | Initialize module |
| `reovim_module_exit` | `(*mut void) -> i32` | Cleanup module |
| `reovim_module_destroy` | `(*mut void) -> void` | Free memory |

### Loading Protocol

The module loader follows this sequence:

1. **Load shared library** via `dlopen()` / `libloading`
2. **Read `REOVIM_MODULE_API_VERSION`** - reject if incompatible
3. **Call `reovim_module_probe()`** - get metadata, check dependencies
4. **Call `reovim_module_entry()`** - create instance
5. **Call `reovim_module_init()`** - initialize with context
6. **Module runs** - handles events, responds to commands
7. **Call `reovim_module_exit()`** - cleanup
8. **Call `reovim_module_destroy()`** - free memory
9. **Unload shared library** via `dlclose()`

### Return Codes

Init and exit functions return:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Defer (init only - try again later) |
| `-1` | Failed/Error |
| `-2` | Panic occurred (Rust modules only) |

## Type Definitions

### ReovimVersion

```c
typedef struct ReovimVersion {
    uint32_t major;
    uint32_t minor;
    uint32_t patch;
} ReovimVersion;
```

- **Size**: 12 bytes
- **Alignment**: 4 bytes

### ReovimModuleProbe

```c
typedef struct ReovimModuleProbe {
    uint8_t id[64];              // Module ID (null-terminated)
    uint8_t name[128];           // Display name (null-terminated)
    ReovimVersion version;       // Module version
    ReovimVersion api_version;   // Required API version
    uint8_t rustc_version[64];   // Compiler version (for ABI checks)
    uint8_t required_deps_count; // Number of required deps (max 8)
    uint8_t required_deps[8][64]; // Required dependency IDs
    uint8_t optional_deps_count; // Number of optional deps (max 8)
    uint8_t optional_deps[8][64]; // Optional dependency IDs
} ReovimModuleProbe;
```

- **Size**: 1308 bytes
- **Alignment**: 4 bytes

## Kernel Services

The kernel provides these functions for module use:

### Version Checking

```c
// Get kernel's ABI version
ReovimVersion reovim_abi_version(void);

// Check compatibility
bool reovim_abi_is_compatible(ReovimVersion required, ReovimVersion provided);
```

### Logging

```c
void reovim_log_info(const char* msg);
void reovim_log_warn(const char* msg);
void reovim_log_error(const char* msg);
void reovim_log_debug(const char* msg);
```

All logging functions handle NULL pointers gracefully.

## Memory Ownership

### Rules

1. **Module owns its instance**: The pointer from `reovim_module_entry()` belongs
   to the module. The kernel will call `reovim_module_destroy()` when done.

2. **Context is borrowed**: The `ctx` pointer in `reovim_module_init()` is only
   valid during that call. Do not store it.

3. **Strings are copied**: All strings in `ReovimModuleProbe` are copied into
   fixed-size buffers. The kernel does not keep references to string data.

4. **No cross-FFI allocation**: Memory allocated on one side of the FFI boundary
   must be freed on the same side.

### Hot Reload State

For hot reload support, additional entry points handle state serialization:

```c
// Check if module supports hot reload
int32_t reovim_module_supports_hot_reload(const void* module);

// Save state (returns allocated buffer via out params)
int32_t reovim_module_save_state(const void* module, uint8_t** out_ptr, size_t* out_len);

// Restore state from buffer
int32_t reovim_module_restore_state(void* module, const uint8_t* data, size_t len);

// Free state buffer from save_state
void reovim_module_free_state(uint8_t* ptr, size_t len);
```

## Example C Module

See `examples/c-module/` for a complete example. Basic structure:

```c
#include <reovim.h>

// Required: API version
const ReovimVersion REOVIM_MODULE_API_VERSION = {0, 2, 0};

// Module state
typedef struct MyModule {
    int initialized;
} MyModule;

// Required: Probe metadata
ReovimModuleProbe reovim_module_probe(void) {
    ReovimModuleProbe probe = {0};

    // Copy ID
    const char* id = "my-module";
    for (int i = 0; id[i] && i < 63; i++) {
        probe.id[i] = id[i];
    }

    // Copy name
    const char* name = "My Module";
    for (int i = 0; name[i] && i < 127; i++) {
        probe.name[i] = name[i];
    }

    probe.version = (ReovimVersion){1, 0, 0};
    probe.api_version = (ReovimVersion){0, 2, 0};

    return probe;
}

// Required: Create instance
void* reovim_module_entry(void) {
    MyModule* m = malloc(sizeof(MyModule));
    m->initialized = 0;
    return m;
}

// Required: Initialize
int32_t reovim_module_init(void* module, const void* ctx) {
    MyModule* m = (MyModule*)module;
    (void)ctx;  // Unused for now

    reovim_log_info("MyModule initializing");
    m->initialized = 1;
    return 0;  // Success
}

// Required: Cleanup
int32_t reovim_module_exit(void* module) {
    MyModule* m = (MyModule*)module;
    reovim_log_info("MyModule exiting");
    m->initialized = 0;
    return 0;  // Success
}

// Required: Free memory
void reovim_module_destroy(void* module) {
    free(module);
}
```

## Building External Modules

### GCC/Clang (Linux)

```bash
gcc -shared -fPIC -o mymodule.so mymodule.c -I/path/to/reovim/lib/drivers/ffi/include
```

### Link with reovim

The module links dynamically at runtime. No static linking required. The
`reovim_*` service functions are resolved when the module is loaded.

## Future Extensions

The following services are planned for future ABI versions:

- **EventBus access**: Subscribe to/publish kernel events
- **Buffer operations**: Read/write buffer contents
- **Command registration**: Register new editor commands
- **Keybinding registration**: Add keybindings

These will be added in a backwards-compatible manner (minor version bump) where
possible, or with an ABI version bump if struct layouts must change.
