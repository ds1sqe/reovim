/*
 * reovim.h - C header for reovim external modules
 *
 * This header defines the ABI for external modules that wish to extend
 * reovim. Modules can be written in C, Haskell, or any language with C FFI.
 *
 * ABI Version: 1.0.0
 * API Version: 0.2.0
 *
 * See docs/architecture/ffi/overview.md for full documentation.
 */

#ifndef REOVIM_H
#define REOVIM_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * Version Constants
 * ============================================================================ */

/**
 * ABI version - binary compatibility.
 *
 * Bumped when struct layouts or function signatures change.
 * Modules compiled against ABI 1.x.y are compatible with ABI 1.x.z (any patch).
 * Modules requiring ABI 1.2.x are compatible with ABI 1.3.x (higher minor).
 */
#define REOVIM_ABI_VERSION_MAJOR 1
#define REOVIM_ABI_VERSION_MINOR 0
#define REOVIM_ABI_VERSION_PATCH 0

/**
 * API version - semantic compatibility.
 *
 * Bumped when trait methods or behavior changes.
 */
#define REOVIM_API_VERSION_MAJOR 0
#define REOVIM_API_VERSION_MINOR 2
#define REOVIM_API_VERSION_PATCH 0

/* ============================================================================
 * Type Definitions
 * ============================================================================ */

/**
 * Semantic version representation.
 *
 * Size: 12 bytes
 * Alignment: 4 bytes
 *
 * Fields are ordered: major, minor, patch (each uint32_t).
 */
typedef struct ReovimVersion {
    uint32_t major;
    uint32_t minor;
    uint32_t patch;
} ReovimVersion;

/**
 * FFI-safe module metadata for discovery.
 *
 * Size: 1308 bytes
 * Alignment: 4 bytes
 *
 * All strings are null-terminated within their fixed buffers.
 * Strings exceeding buffer size are truncated (not an error).
 */
typedef struct ReovimModuleProbe {
    /** Module ID (null-terminated, max 63 chars + nul) */
    uint8_t id[64];

    /** Module name (null-terminated, max 127 chars + nul) */
    uint8_t name[128];

    /** Module version */
    ReovimVersion version;

    /** Required kernel API version */
    ReovimVersion api_version;

    /** Rustc version used to compile (null-terminated, max 63 chars + nul) */
    uint8_t rustc_version[64];

    /** Number of required dependencies (max 8) */
    uint8_t required_deps_count;

    /** Required dependency IDs (null-terminated strings) */
    uint8_t required_deps[8][64];

    /** Number of optional dependencies (max 8) */
    uint8_t optional_deps_count;

    /** Optional dependency IDs (null-terminated strings) */
    uint8_t optional_deps[8][64];
} ReovimModuleProbe;

/* ============================================================================
 * Module Entry Points
 * ============================================================================
 *
 * External modules MUST export these symbols. The reovim loader calls them
 * in this order:
 *
 * 1. Read REOVIM_MODULE_API_VERSION for pre-load compatibility check
 * 2. Call reovim_module_probe() to get metadata
 * 3. Call reovim_module_entry() to create instance
 * 4. Call reovim_module_init() to initialize
 * 5. ... module runs ...
 * 6. Call reovim_module_exit() to cleanup
 * 7. Call reovim_module_destroy() to free memory
 */

/**
 * Static API version for pre-load check.
 *
 * Export this symbol with your module's required API version.
 * The loader reads this BEFORE calling any functions.
 *
 * Example:
 *   const ReovimVersion REOVIM_MODULE_API_VERSION = {0, 2, 0};
 */
/* extern const ReovimVersion REOVIM_MODULE_API_VERSION; */

/**
 * Return module metadata without full instantiation.
 *
 * Called by loader to discover module identity and dependencies.
 * May create a temporary instance internally.
 *
 * @return Populated ReovimModuleProbe struct
 */
/* ReovimModuleProbe reovim_module_probe(void); */

/**
 * Create module instance.
 *
 * Allocates and returns an opaque pointer to the module struct.
 * The module is not yet initialized - call reovim_module_init() next.
 *
 * @return Opaque pointer to module instance (caller must not dereference)
 */
/* void* reovim_module_entry(void); */

/**
 * Initialize module.
 *
 * @param module  Opaque pointer from reovim_module_entry()
 * @param ctx     Pointer to ModuleContext (treat as opaque)
 *
 * @return 0 = success, 1 = defer (try again later), -1 = failed, -2 = panic
 */
/* int32_t reovim_module_init(void* module, const void* ctx); */

/**
 * Cleanup module before unload.
 *
 * @param module  Opaque pointer from reovim_module_entry()
 *
 * @return 0 = success, -1 = error, -2 = panic
 */
/* int32_t reovim_module_exit(void* module); */

/**
 * Free module memory.
 *
 * Called after reovim_module_exit(). Must not be called twice.
 *
 * @param module  Opaque pointer from reovim_module_entry()
 */
/* void reovim_module_destroy(void* module); */

/* ============================================================================
 * Kernel Services
 * ============================================================================
 *
 * These functions are provided by the kernel for module use.
 */

/**
 * Get the kernel's ABI version.
 *
 * Call this to check compatibility before using other services.
 *
 * @return Current kernel ABI version
 */
ReovimVersion reovim_abi_version(void);

/**
 * Check if two ABI versions are compatible.
 *
 * Compatibility rules:
 * - Major version must match exactly
 * - Required minor must be <= provided minor
 * - Patch version is ignored
 *
 * @param required  The version the module requires
 * @param provided  The version the kernel provides
 *
 * @return true if compatible, false otherwise
 */
bool reovim_abi_is_compatible(ReovimVersion required, ReovimVersion provided);

/**
 * Log an info-level message.
 *
 * @param msg  Null-terminated message string (may be NULL)
 */
void reovim_log_info(const char* msg);

/**
 * Log a warning-level message.
 *
 * @param msg  Null-terminated message string (may be NULL)
 */
void reovim_log_warn(const char* msg);

/**
 * Log an error-level message.
 *
 * @param msg  Null-terminated message string (may be NULL)
 */
void reovim_log_error(const char* msg);

/**
 * Log a debug-level message.
 *
 * @param msg  Null-terminated message string (may be NULL)
 */
void reovim_log_debug(const char* msg);

/* ============================================================================
 * Helper Macros
 * ============================================================================ */

/**
 * Create a ReovimVersion literal.
 */
#define REOVIM_VERSION(major, minor, patch) \
    ((ReovimVersion){(major), (minor), (patch)})

/**
 * Check if kernel ABI is compatible with required version.
 *
 * Usage:
 *   if (!REOVIM_CHECK_ABI(1, 0, 0)) {
 *       return -1;  // Incompatible
 *   }
 */
#define REOVIM_CHECK_ABI(major, minor, patch) \
    reovim_abi_is_compatible(REOVIM_VERSION((major), (minor), (patch)), reovim_abi_version())

#ifdef __cplusplus
}
#endif

#endif /* REOVIM_H */
