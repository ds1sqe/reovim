/**
 * GHC Runtime System Initialization Shim
 *
 * This file provides automatic initialization and cleanup of the GHC
 * Runtime System (RTS) when the shared library is loaded/unloaded.
 *
 * The GHC RTS must be initialized before ANY Haskell code can execute.
 * Since reovim's module loader calls probe functions before entry(),
 * we use GCC's constructor/destructor attributes to ensure the RTS
 * is ready when dlopen() returns.
 *
 * References:
 * - https://downloads.haskell.org/ghc/latest/docs/users_guide/shared_libs.html
 * - https://well-typed.com/blog/2009/05/buildings-plugins-as-haskell-shared-libs/
 */

#include <stddef.h>  /* for NULL */
#include <stdio.h>   /* for fprintf */
#include <HsFFI.h>

/**
 * Initialize GHC RTS when library is loaded.
 *
 * The __attribute__((constructor)) causes this function to be called
 * automatically when the shared library is loaded via dlopen(), before
 * dlopen() returns to the caller.
 *
 * This ensures the RTS is initialized before reovim calls any FFI
 * functions like reovim_module_probe or reovim_module_api_version_ptr.
 */
static void reovim_rts_init(void) __attribute__((constructor));
static void reovim_rts_init(void)
{
    /* Dummy argc/argv for hs_init - required but not used */
    static char *argv[] = { "reovim-haskell-module", NULL };
    static char **argv_ = argv;
    static int argc = 1;

    hs_init(&argc, &argv_);
}

/**
 * Shutdown GHC RTS when library is unloaded.
 *
 * The __attribute__((destructor)) causes this function to be called
 * automatically when the shared library is unloaded via dlclose(),
 * or when the process exits.
 *
 * Note: GHC does not support reinitializing the RTS after hs_exit().
 * Once a Haskell module is unloaded, it cannot be reloaded in the
 * same process.
 */
static void reovim_rts_exit(void) __attribute__((destructor));
static void reovim_rts_exit(void)
{
    hs_exit();
}

/*
 * Logging stubs - these should be provided by the reovim kernel.
 * For standalone testing, we provide no-op implementations.
 *
 * TODO: Issue #305 - Implement proper kernel logging FFI exports
 */

void reovim_log_trace(const char *msg) {
    fprintf(stderr, "[TRACE] %s\n", msg);
}

void reovim_log_debug(const char *msg) {
    fprintf(stderr, "[DEBUG] %s\n", msg);
}

void reovim_log_info(const char *msg) {
    fprintf(stderr, "[INFO] %s\n", msg);
}

void reovim_log_warn(const char *msg) {
    fprintf(stderr, "[WARN] %s\n", msg);
}

void reovim_log_error(const char *msg) {
    fprintf(stderr, "[ERROR] %s\n", msg);
}
