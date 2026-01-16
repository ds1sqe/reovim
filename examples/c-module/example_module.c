/*
 * example_module.c - Example reovim module in C
 *
 * This demonstrates the minimal implementation of a reovim external module.
 * It implements all required entry points and uses kernel services.
 *
 * Build:
 *   make
 *
 * Or manually:
 *   gcc -shared -fPIC -o example_module.so example_module.c \
 *       -I../../lib/drivers/ffi/include
 */

#include <stdlib.h>
#include <string.h>

#include "reovim.h"

/* ============================================================================
 * Required: API Version Declaration
 * ============================================================================ */

/**
 * The API version this module requires.
 *
 * The loader reads this BEFORE calling any functions to check compatibility.
 * This module requires API 0.2.0 (current version).
 */
const ReovimVersion REOVIM_MODULE_API_VERSION = {0, 2, 0};

/* ============================================================================
 * Module State
 * ============================================================================ */

/**
 * Module-specific state.
 *
 * This is opaque to the kernel - only your module knows its layout.
 */
typedef struct ExampleModule {
    int initialized;
    int counter;
} ExampleModule;

/* ============================================================================
 * Required Entry Points
 * ============================================================================ */

/**
 * Return module metadata for discovery.
 *
 * Called by loader to get module identity and dependencies without
 * fully instantiating the module.
 */
ReovimModuleProbe reovim_module_probe(void) {
    ReovimModuleProbe probe;
    memset(&probe, 0, sizeof(probe));

    /* Module ID - used for dependency resolution */
    const char* id = "example-c";
    size_t id_len = strlen(id);
    if (id_len > 63) id_len = 63;
    memcpy(probe.id, id, id_len);

    /* Human-readable name */
    const char* name = "Example C Module";
    size_t name_len = strlen(name);
    if (name_len > 127) name_len = 127;
    memcpy(probe.name, name, name_len);

    /* Module version */
    probe.version.major = 1;
    probe.version.minor = 0;
    probe.version.patch = 0;

    /* Required API version */
    probe.api_version.major = 0;
    probe.api_version.minor = 2;
    probe.api_version.patch = 0;

    /* No dependencies for this example */
    probe.required_deps_count = 0;
    probe.optional_deps_count = 0;

    return probe;
}

/**
 * Create and return a new module instance.
 *
 * The returned pointer is opaque to the kernel. Only your module
 * knows how to interpret it.
 */
void* reovim_module_entry(void) {
    ExampleModule* module = malloc(sizeof(ExampleModule));
    if (module == NULL) {
        return NULL;
    }

    module->initialized = 0;
    module->counter = 0;

    return module;
}

/**
 * Initialize the module.
 *
 * Called after reovim_module_entry(). The ctx parameter provides
 * access to kernel services (currently opaque).
 *
 * Return codes:
 *   0  = Success
 *   1  = Defer (try again later)
 *   -1 = Failed
 */
int32_t reovim_module_init(void* module_ptr, const void* ctx) {
    ExampleModule* module = (ExampleModule*)module_ptr;

    /* ctx is currently opaque - future versions will provide typed access */
    (void)ctx;

    /* Check ABI compatibility */
    if (!REOVIM_CHECK_ABI(1, 0, 0)) {
        reovim_log_error("ABI version mismatch");
        return -1;
    }

    reovim_log_info("Example C module initializing");

    module->initialized = 1;
    module->counter = 42;

    reovim_log_debug("Counter initialized to 42");

    return 0;  /* Success */
}

/**
 * Cleanup the module before unload.
 *
 * Called before reovim_module_destroy(). Release any resources here.
 *
 * Return codes:
 *   0  = Success
 *   -1 = Error
 */
int32_t reovim_module_exit(void* module_ptr) {
    ExampleModule* module = (ExampleModule*)module_ptr;

    reovim_log_info("Example C module exiting");

    module->initialized = 0;
    module->counter = 0;

    return 0;  /* Success */
}

/**
 * Free module memory.
 *
 * Called after reovim_module_exit(). Free all allocated memory.
 */
void reovim_module_destroy(void* module_ptr) {
    if (module_ptr != NULL) {
        free(module_ptr);
    }
}

/* ============================================================================
 * Optional: Hot Reload Support
 * ============================================================================ */

/**
 * Check if module supports hot reload.
 *
 * Return 1 for yes, 0 for no.
 */
int32_t reovim_module_supports_hot_reload(const void* module_ptr) {
    (void)module_ptr;
    return 0;  /* This example doesn't support hot reload */
}

/**
 * Save module state for hot reload.
 *
 * Not implemented in this example.
 */
int32_t reovim_module_save_state(
    const void* module_ptr,
    uint8_t** out_ptr,
    size_t* out_len
) {
    (void)module_ptr;
    *out_ptr = NULL;
    *out_len = 0;
    return 1;  /* No state to save */
}

/**
 * Restore module state after hot reload.
 *
 * Not implemented in this example.
 */
int32_t reovim_module_restore_state(
    void* module_ptr,
    const uint8_t* data,
    size_t len
) {
    (void)module_ptr;
    (void)data;
    (void)len;
    return -1;  /* Not supported */
}

/**
 * Free state buffer.
 *
 * Not implemented in this example.
 */
void reovim_module_free_state(uint8_t* ptr, size_t len) {
    (void)ptr;
    (void)len;
    /* Nothing to free */
}
