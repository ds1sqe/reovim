/*
 * test_stubs.c - Stub implementations for FFI safety tests
 *
 * These stubs allow the test executable to link without reovim.
 * They provide no-op implementations of kernel services.
 */

#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>

/* Logging stubs - just print to stdout for testing */
void reovim_log_info(const char* msg) {
    if (msg) printf("[INFO] %s\n", msg);
}

void reovim_log_warn(const char* msg) {
    if (msg) printf("[WARN] %s\n", msg);
}

void reovim_log_error(const char* msg) {
    if (msg) printf("[ERROR] %s\n", msg);
}

void reovim_log_debug(const char* msg) {
    if (msg) printf("[DEBUG] %s\n", msg);
}

/* Version struct */
typedef struct ReovimVersion {
    uint32_t major;
    uint32_t minor;
    uint32_t patch;
} ReovimVersion;

/* Version stub - fills provided buffer with ABI version */
void reovim_abi_version_ptr(ReovimVersion* out) {
    if (out) {
        out->major = 1;
        out->minor = 0;
        out->patch = 0;
    }
}

/* Compatibility check stub */
bool reovim_abi_is_compatible(ReovimVersion required, ReovimVersion provided) {
    return required.major == provided.major &&
           required.minor <= provided.minor;
}
