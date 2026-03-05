/*
 * reovim.h - C header for reovim external modules
 *
 * This header defines the ABI for external modules that wish to extend
 * reovim. Modules can be written in C, Haskell, or any language with C FFI.
 *
 * ABI Version: 1.1.0
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
 *
 * History:
 *   1.0.0 - Initial stable ABI (ModuleProbe, logging, timers)
 *   1.1.0 - Buffer, window, mode, command, and event APIs
 */
#define REOVIM_ABI_VERSION_MAJOR 1
#define REOVIM_ABI_VERSION_MINOR 1
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
 * Error Codes
 * ============================================================================
 *
 * All FFI functions return i32 status codes.
 * - 0 (REOVIM_OK): Success
 * - Negative values: Errors
 * - Positive values: Non-error status (e.g., command results)
 */

#define REOVIM_OK               0
#define REOVIM_ERR_NO_RUNTIME   (-1)   /* No active RuntimeGuard */
#define REOVIM_ERR_NULL_PTR     (-2)   /* Required pointer was null */
#define REOVIM_ERR_NOT_FOUND    (-3)   /* Buffer/window/mode not found */
#define REOVIM_ERR_INVALID_UTF8 (-4)   /* String is not valid UTF-8 */
#define REOVIM_ERR_OUT_OF_RANGE (-5)   /* Line/column index out of range */
#define REOVIM_ERR_LAST_BUFFER  (-6)   /* Cannot delete last buffer */
#define REOVIM_ERR_LAST_WINDOW  (-7)   /* Cannot close last window */
#define REOVIM_ERR_FAILED       (-8)   /* Generic failure */
#define REOVIM_ERR_PANIC        (-9)   /* Rust panic caught at FFI boundary */
#define REOVIM_ERR_NO_INIT_CTX  (-10)  /* No active InitGuard */

/* Command result codes (positive, returned by reovim_execute_command) */
#define REOVIM_CMD_SUCCESS      0
#define REOVIM_CMD_QUIT         1
#define REOVIM_CMD_FORCE_QUIT   2
#define REOVIM_CMD_DETACH       3
#define REOVIM_CMD_ERROR        4

/* ============================================================================
 * Type Definitions
 * ============================================================================ */

/**
 * Semantic version representation.
 *
 * Size: 12 bytes, Alignment: 4 bytes
 */
typedef struct ReovimVersion {
    uint32_t major;
    uint32_t minor;
    uint32_t patch;
} ReovimVersion;

/**
 * A line/column position in a buffer.
 *
 * Both line and column are zero-based.
 *
 * Size: 8 bytes, Alignment: 4 bytes
 */
typedef struct ReovimPosition {
    uint32_t line;      /* Zero-based line number */
    uint32_t column;    /* Zero-based column number */
} ReovimPosition;

/**
 * Result of a string read operation.
 *
 * When a function fills a caller-owned buffer with a string, this struct
 * reports the status and actual length. If length > buf_len, the string
 * was truncated.
 *
 * Size: 8 bytes, Alignment: 4 bytes
 */
typedef struct ReovimStringResult {
    int32_t  status;    /* REOVIM_OK or error code */
    uint32_t length;    /* Actual string length (may exceed buf_len) */
} ReovimStringResult;

/**
 * Parsed command arguments passed to command callbacks.
 *
 * Size: 28 bytes, Alignment: 4 bytes
 */
typedef struct ReovimCommandArgs {
    int32_t  has_count;     /* 0 = no count, 1 = count present */
    uint32_t count;         /* Count value (valid if has_count == 1) */
    int32_t  has_register;  /* 0 = no register, 1 = register present */
    uint8_t  register_;     /* Register character (valid if has_register == 1) */
    uint8_t  pad[3];        /* Padding for alignment */
    int32_t  has_cursor;    /* 0 = no cursor, 1 = cursor present */
    ReovimPosition cursor;  /* Cursor position (valid if has_cursor == 1) */
} ReovimCommandArgs;

/** Opaque buffer ID. */
typedef uint64_t ReovimBufferId;

/** Opaque window ID. */
typedef uint64_t ReovimWindowId;

/**
 * Subscription handle for event callbacks.
 *
 * id == 0 means null/invalid.
 *
 * Size: 8 bytes, Alignment: 8 bytes
 */
typedef struct ReovimSubscriptionHandle {
    uint64_t id;
} ReovimSubscriptionHandle;

/**
 * FFI-safe module metadata for discovery.
 *
 * Size: 1308 bytes, Alignment: 4 bytes
 *
 * All strings are null-terminated within their fixed buffers.
 */
typedef struct ReovimModuleProbe {
    uint8_t id[64];                     /* Module ID (max 63 chars + nul) */
    uint8_t name[128];                  /* Module name (max 127 chars + nul) */
    ReovimVersion version;              /* Module version */
    ReovimVersion api_version;          /* Required kernel API version */
    uint8_t rustc_version[64];          /* Rustc version (max 63 chars + nul) */
    uint8_t required_deps_count;        /* Number of required deps (max 8) */
    uint8_t required_deps[8][64];       /* Required dependency IDs */
    uint8_t optional_deps_count;        /* Number of optional deps (max 8) */
    uint8_t optional_deps[8][64];       /* Optional dependency IDs */
} ReovimModuleProbe;

/**
 * Command registration info.
 *
 * Passed to reovim_register_command() during module init.
 */
typedef struct ReovimCommandRegistration {
    const char* id;             /* Command ID in "module:command" format */
    const char* description;    /* Human-readable description */
    int32_t (*callback)(void* user_data, const ReovimCommandArgs* args);
    void* user_data;            /* Opaque pointer passed to callback */
} ReovimCommandRegistration;

/* ============================================================================
 * Callback Types
 * ============================================================================ */

/** Timer callback. */
typedef void (*ReovimTimerCallback)(void* user_data);

/** Command callback. Returns REOVIM_CMD_* result code. */
typedef int32_t (*ReovimCommandCallback)(void* user_data, const ReovimCommandArgs* args);

/**
 * Event callback.
 *
 * @param user_data       Opaque pointer from subscription
 * @param event_type      Null-terminated event type name
 * @param event_data      Event-specific data (NULL in v1)
 * @param event_data_len  Length of event_data (0 in v1)
 */
typedef void (*ReovimEventCallback)(
    void* user_data,
    const char* event_type,
    const void* event_data,
    uint32_t event_data_len
);

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
 * 4. Call reovim_module_init(module, ctx)
 *    - During init, module may call:
 *      - reovim_register_command() to register command handlers
 *      - reovim_subscribe_event() to subscribe to events
 * 5. Module runs (engine calls registered command callbacks)
 *    - During callbacks, module may call:
 *      - Buffer API (reovim_buffer_line, reovim_insert_text, ...)
 *      - Window API (reovim_active_window, reovim_cursor_position, ...)
 *      - Mode API (reovim_current_mode, reovim_push_mode, ...)
 *      - Command API (reovim_execute_command)
 * 6. Call reovim_module_exit() to cleanup
 * 7. Call reovim_module_destroy() to free memory
 */

/* extern const ReovimVersion REOVIM_MODULE_API_VERSION; */
/* ReovimModuleProbe reovim_module_probe(void); */
/* void* reovim_module_entry(void); */
/* int32_t reovim_module_init(void* module, const void* ctx); */
/* int32_t reovim_module_exit(void* module); */
/* void reovim_module_destroy(void* module); */

/* ============================================================================
 * Kernel Services - Version
 * ============================================================================ */

/** Get the kernel's ABI version. */
ReovimVersion reovim_abi_version(void);

/**
 * Check if two ABI versions are compatible.
 *
 * - Major must match exactly
 * - Required minor <= provided minor
 * - Patch is ignored
 */
bool reovim_abi_is_compatible(ReovimVersion required, ReovimVersion provided);

/* ============================================================================
 * Kernel Services - Logging
 * ============================================================================ */

void reovim_log_info(const char* msg);
void reovim_log_warn(const char* msg);
void reovim_log_error(const char* msg);
void reovim_log_debug(const char* msg);

/* ============================================================================
 * Kernel Services - Timers
 * ============================================================================ */

typedef struct ReovimTimerHandle {
    uint64_t id;    /* 0 = invalid */
} ReovimTimerHandle;

ReovimTimerHandle reovim_schedule_delayed(
    uint64_t delay_ms,
    ReovimTimerCallback callback,
    void* user_data
);

ReovimTimerHandle reovim_schedule_periodic(
    uint64_t interval_ms,
    ReovimTimerCallback callback,
    void* user_data
);

bool reovim_cancel_timer(ReovimTimerHandle handle);
bool reovim_timer_is_pending(ReovimTimerHandle handle);
size_t reovim_timer_count(void);

/* ============================================================================
 * Kernel Services - Buffer API (ABI 1.1.0)
 * ============================================================================
 *
 * Buffer functions require an active RuntimeGuard (must be called during
 * a command callback). Returns REOVIM_ERR_NO_RUNTIME otherwise.
 */

/**
 * Get the active buffer ID.
 *
 * @param out_id  Receives the buffer ID on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, REOVIM_ERR_NOT_FOUND, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_active_buffer(ReovimBufferId* out_id);

/**
 * Get a single line from a buffer.
 *
 * Writes the line content to buf. If the line is longer than buf_len,
 * it is truncated but out_result->length reports the full length.
 *
 * @param buffer_id   Buffer to read from
 * @param line        Zero-based line number
 * @param buf         Caller-owned output buffer (may be NULL to query length)
 * @param buf_len     Size of buf in bytes
 * @param out_result  Receives status and actual string length
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, REOVIM_ERR_OUT_OF_RANGE, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_buffer_line(
    ReovimBufferId buffer_id,
    uint32_t line,
    uint8_t* buf,
    uint32_t buf_len,
    ReovimStringResult* out_result
);

/**
 * Get the number of lines in a buffer.
 *
 * @param buffer_id  Buffer to query
 * @param out_count  Receives line count on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, REOVIM_ERR_NOT_FOUND, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_buffer_line_count(ReovimBufferId buffer_id, uint32_t* out_count);

/**
 * Get the byte length of a single line.
 *
 * @param buffer_id  Buffer to query
 * @param line       Zero-based line number
 * @param out_len    Receives line length in bytes on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, REOVIM_ERR_OUT_OF_RANGE, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_buffer_line_len(ReovimBufferId buffer_id, uint32_t line, uint32_t* out_len);

/**
 * Get text in a range from a buffer.
 *
 * @param buffer_id   Buffer to read from
 * @param start       Start position (inclusive)
 * @param end         End position (exclusive)
 * @param buf         Caller-owned output buffer
 * @param buf_len     Size of buf in bytes
 * @param out_result  Receives status and actual string length
 * @return REOVIM_OK or error code
 */
int32_t reovim_buffer_text_range(
    ReovimBufferId buffer_id,
    ReovimPosition start,
    ReovimPosition end,
    uint8_t* buf,
    uint32_t buf_len,
    ReovimStringResult* out_result
);

/**
 * Get the full content of a buffer.
 *
 * @param buffer_id   Buffer to read from
 * @param buf         Caller-owned output buffer
 * @param buf_len     Size of buf in bytes
 * @param out_result  Receives status and actual string length
 * @return REOVIM_OK or error code
 */
int32_t reovim_buffer_content(
    ReovimBufferId buffer_id,
    uint8_t* buf,
    uint32_t buf_len,
    ReovimStringResult* out_result
);

/**
 * Insert text at a position in a buffer.
 *
 * @param buffer_id  Target buffer
 * @param pos        Insertion position
 * @param text       Null-terminated text to insert
 * @return REOVIM_OK or error code
 */
int32_t reovim_insert_text(
    ReovimBufferId buffer_id,
    ReovimPosition pos,
    const char* text
);

/**
 * Delete a range of text from a buffer.
 *
 * @param buffer_id  Target buffer
 * @param start      Start position (inclusive)
 * @param end        End position (exclusive)
 * @return REOVIM_OK or error code
 */
int32_t reovim_delete_range(
    ReovimBufferId buffer_id,
    ReovimPosition start,
    ReovimPosition end
);

/**
 * Create a new buffer.
 *
 * @param name     Buffer name (null-terminated, may be NULL for unnamed)
 * @param content  Initial content (null-terminated, may be NULL for empty)
 * @param out_id   Receives the new buffer ID on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, REOVIM_ERR_INVALID_UTF8,
 *         or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_create_buffer(
    const char* name,
    const char* content,
    ReovimBufferId* out_id
);

/**
 * Delete a buffer.
 *
 * @param buffer_id  Buffer to delete
 * @return REOVIM_OK, REOVIM_ERR_NOT_FOUND, REOVIM_ERR_LAST_BUFFER, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_delete_buffer(ReovimBufferId buffer_id);

/* ============================================================================
 * Kernel Services - Window API (ABI 1.1.0)
 * ============================================================================ */

/**
 * Get the active window ID.
 *
 * @param out_id  Receives the window ID on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, REOVIM_ERR_NOT_FOUND, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_active_window(ReovimWindowId* out_id);

/**
 * Get the cursor position from the active window.
 *
 * @param out_pos  Receives the cursor position on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, REOVIM_ERR_NOT_FOUND, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_cursor_position(ReovimPosition* out_pos);

/**
 * Get the number of windows.
 *
 * @param out_count  Receives window count on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_window_count(uint32_t* out_count);

/**
 * Get the buffer displayed in a window.
 *
 * @param window_id   Window to query
 * @param out_buffer  Receives the buffer ID on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, REOVIM_ERR_NOT_FOUND, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_window_buffer(ReovimWindowId window_id, ReovimBufferId* out_buffer);

/**
 * Create a new window.
 *
 * @param buffer_id  Buffer to display (0 = no buffer)
 * @param out_id     Receives the new window ID on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_create_window(ReovimBufferId buffer_id, ReovimWindowId* out_id);

/**
 * Close a window.
 *
 * @param window_id  Window to close
 * @return REOVIM_OK, REOVIM_ERR_NOT_FOUND, REOVIM_ERR_LAST_WINDOW, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_close_window(ReovimWindowId window_id);

/**
 * Focus a window.
 *
 * @param window_id  Window to focus
 * @return REOVIM_OK, REOVIM_ERR_NOT_FOUND, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_focus_window(ReovimWindowId window_id);

/* ============================================================================
 * Kernel Services - Mode API (ABI 1.1.0)
 * ============================================================================
 *
 * Mode IDs are strings in "module:name" format (e.g., "vim:normal").
 */

/**
 * Get the current mode as a string.
 *
 * Writes the mode ID (e.g., "vim:normal") to the caller-owned buffer.
 *
 * @param buf         Caller-owned output buffer
 * @param buf_len     Size of buf in bytes
 * @param out_result  Receives status and actual string length
 * @return REOVIM_OK or error code
 */
int32_t reovim_current_mode(uint8_t* buf, uint32_t buf_len, ReovimStringResult* out_result);

/**
 * Get the mode stack depth.
 *
 * @param out_depth  Receives depth on success
 * @return REOVIM_OK, REOVIM_ERR_NULL_PTR, or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_mode_depth(uint32_t* out_depth);

/**
 * Push a mode onto the mode stack.
 *
 * @param mode_id  Null-terminated "module:name" string
 * @return REOVIM_OK or error code
 */
int32_t reovim_push_mode(const char* mode_id);

/**
 * Pop the current mode from the stack.
 *
 * Returns to the previous mode. Cannot pop the home mode.
 *
 * @return REOVIM_OK, REOVIM_ERR_FAILED (home mode), or REOVIM_ERR_NO_RUNTIME
 */
int32_t reovim_pop_mode(void);

/**
 * Replace the current mode (pop + push atomically).
 *
 * @param mode_id  Null-terminated "module:name" string
 * @return REOVIM_OK or error code
 */
int32_t reovim_set_mode(const char* mode_id);

/* ============================================================================
 * Kernel Services - Command API (ABI 1.1.0)
 * ============================================================================ */

/**
 * Execute a command by ID string.
 *
 * @param cmd_id    Null-terminated "module:command" string
 * @param count     Count prefix (-1 for none)
 * @param register_ Register character (0 for none)
 * @return REOVIM_CMD_* result or REOVIM_ERR_* error code
 */
int32_t reovim_execute_command(const char* cmd_id, int32_t count, uint8_t register_);

/**
 * Register a command handler.
 *
 * Must be called during module init (InitGuard active).
 *
 * @param reg  Pointer to registration info
 * @return REOVIM_OK or error code
 */
int32_t reovim_register_command(const ReovimCommandRegistration* reg);

/* ============================================================================
 * Kernel Services - Event API (ABI 1.1.0)
 * ============================================================================
 *
 * Event names are strings (e.g., "buffer:changed", "mode:changed").
 * In v1, event_data is always NULL and event_data_len is always 0.
 */

/**
 * Subscribe to an event type.
 *
 * Must be called during module init (InitGuard active).
 *
 * @param event_type  Null-terminated event type name
 * @param callback    Function to call when event fires
 * @param user_data   Opaque pointer passed to callback
 * @param priority    Subscription priority (lower = earlier)
 * @return Subscription handle (id == 0 on error)
 */
ReovimSubscriptionHandle reovim_subscribe_event(
    const char* event_type,
    ReovimEventCallback callback,
    void* user_data,
    uint32_t priority
);

/**
 * Unsubscribe from an event.
 *
 * Must be called during module init (InitGuard active).
 *
 * @param handle  Handle from reovim_subscribe_event
 * @return REOVIM_OK, REOVIM_ERR_NOT_FOUND, or REOVIM_ERR_NO_INIT_CTX
 */
int32_t reovim_unsubscribe_event(ReovimSubscriptionHandle handle);

/* ============================================================================
 * Helper Macros
 * ============================================================================ */

/** Create a ReovimVersion literal. */
#define REOVIM_VERSION(major, minor, patch) \
    ((ReovimVersion){(major), (minor), (patch)})

/** Check if kernel ABI is compatible with required version. */
#define REOVIM_CHECK_ABI(major, minor, patch) \
    reovim_abi_is_compatible(REOVIM_VERSION((major), (minor), (patch)), reovim_abi_version())

/** Null subscription handle. */
#define REOVIM_NULL_SUBSCRIPTION ((ReovimSubscriptionHandle){0})

#ifdef __cplusplus
}
#endif

#endif /* REOVIM_H */
