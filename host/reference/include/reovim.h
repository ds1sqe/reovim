#ifndef RV_ABI_H
#define RV_ABI_H

#include <stddef.h>
#include <stdint.h>

/* Exact experimental ABI. Example: advertise RV_ABI_VERSION at entry. */
#define RV_ABI_VERSION { 0, 1, 0, 0 }
typedef struct rv_version {
    uint32_t major, minor, patch;
    uint32_t reserved; /* Must be zero. */
} rv_version;

/* Example: reject an entry before reading its tables on mismatch. */
static inline int rv_abi_compatible(rv_version version) {
    const rv_version current = RV_ABI_VERSION;
    return version.major == current.major && version.minor == current.minor
        && version.patch == current.patch && version.reserved == 0;
}

/* Callback statuses, not effect certainty; unknown machine errors survive. */
enum {
    RV_OK = 0, RV_NOT_READY = 1, RV_CLOSED = 2, RV_BUSY = 3,
    RV_INVALID = 4, RV_DENIED = 5, RV_UNSUPPORTED = 6, RV_PRESSURE = 7,
    RV_CONFLICT = 8, RV_UNAVAILABLE = 9, RV_CANCELLED = 10,
    RV_PROVIDER = 11, RV_GAP = 12, RV_INCOMPLETE = 13
};
enum { RV_POLL_PENDING = 0, RV_POLL_READY = 1 };
enum { RV_CANCEL_ACCEPTED = 0, RV_CANCEL_TOO_LATE = 1 };
enum { RV_OUTCOME_OK = 0, RV_OUTCOME_FAILED = 1, RV_OUTCOME_UNKNOWN = 2 };
enum { RV_READ = 1, RV_WRITE = 2, RV_EXEC = 4 };

/* Caller-scoped access; copying a token neither grants nor retains it. */
typedef uint64_t rv_access;
/* Copied diagnostic/subject identity; e.g. not accepted as access. */
typedef struct rv_identity { uint8_t epoch[16]; uint64_t serial; } rv_identity;
/* Borrowed spans; e.g. null with length zero denotes Unit/no slots. */
typedef struct rv_bytes { const uint8_t *ptr; size_t len; } rv_bytes;
typedef struct rv_refs { const rv_access *ptr; size_t len; } rv_refs;
/* A declared value; e.g. Ref bytes contain an index into refs. */
typedef struct rv_args { rv_bytes data; rv_refs refs; } rv_args;
/* Optional detail is all zero when absent; it never supplies recovery access. */
typedef struct rv_error { uint32_t code; rv_identity detail; } rv_error;

/* Poll borrows wake. Example: retain before asynchronous use, release after
 * the last signal. Callbacks are nonnull and may be called across threads. */
typedef struct rv_wake {
    void *context;
    void (*wake)(void *);
    void (*retain)(void *);
    void (*release)(void *);
} rv_wake;

/* One owned generation. Example: take -> validate/copy/adopt -> release(owner).
 * UNKNOWN has declared progress and no returned access; FAILED has no value.
 * Result release and request finish are separate lifetime obligations. */
typedef struct rv_outcome {
    uint32_t status;
    uint32_t activity; /* Known READ=1, WRITE=2, STATUS=4 within declaration. */
    rv_error error;
    rv_args value;
    void *owner;
    void (*release)(void *);
} rv_outcome;

/* Example: poll observes readiness; take transfers once per generation.
 * Reconcile after taken UNKNOWN refines the same effect, never re-executes.
 * Finish OK invalidates context; Busy leaves it owned and progressable.
 * bytes is exactly sizeof(rv_pending_ops); only reconcile may be null. */
typedef struct rv_pending_ops {
    size_t bytes;
    uint32_t (*poll)(void *, rv_wake);
    uint32_t (*take)(void *, rv_outcome *);
    uint32_t (*cancel)(void *);
    uint32_t (*reconcile)(void *);
    uint32_t (*finish)(void *);
} rv_pending_ops;
/* Owned until finish succeeds; e.g. dropping a delivery view cannot free it. */
typedef struct rv_pending {
    void *context;
    const rv_pending_ops *ops;
} rv_pending;

/* Begin OK transfers pending. Error fills error, leaves output untouched and
 * proves no effect/retained input from begin. Copy borrowed input before return.
 * Example: exec has no command; ioctl takes a bound interface's nonzero command.
 * Every sized table requires exact sizeof, not compatibility negotiation. */
typedef struct rv_exec_ops {
    size_t bytes;
    uint32_t (*exec)(void *, const rv_args *, rv_pending *, rv_error *);
} rv_exec_ops;
typedef struct rv_control_ops {
    size_t bytes;
    uint32_t (*ioctl)(void *, uint32_t, const rv_args *, rv_pending *, rv_error *);
} rv_control_ops;

typedef struct rv_provider rv_provider;
typedef struct rv_edit_ops rv_edit_ops;
typedef struct rv_factory_ops rv_factory_ops;
typedef struct rv_open_ops rv_open_ops;
typedef struct rv_io_ops rv_io_ops;
typedef struct rv_namespace_ops rv_namespace_ops;
typedef struct rv_creation rv_creation;
typedef struct rv_write rv_write;

/* Example: dispatch selects the table from the retained canonical declaration.
 * Missing families are null; ordinary callbacks cannot install Buffer edits. */
typedef struct rv_interface {
    size_t bytes;
    void *context;
    rv_bytes declaration;
    const rv_exec_ops *exec;
    const rv_control_ops *control;
    const rv_edit_ops *edit;
    const rv_factory_ops *factory;
} rv_interface;

/* Stage takes provider ownership only on OK. Example: code remains pinned
 * through stop, all request/result cleanup, and the final destroy callback. */
struct rv_provider {
    size_t bytes;
    void *context;
    const rv_interface *interfaces;
    size_t interface_count;
    const rv_open_ops *open;
    const rv_io_ops *io;
    const rv_namespace_ops *namespace_ops;
    uint32_t (*stop)(void *);
    void (*destroy)(void *);
};
/* Scoped publication; e.g. relative paths cannot escape the granted mount. */
typedef struct rv_registrar {
    size_t bytes;
    void *context;
    uint32_t (*stage)(void *, rv_bytes, const rv_provider *);
} rv_registrar;

/* Example: History retains private immutable state; snapshot creates an inert
 * readonly provider without I/O/waiting. Its output transfers only on OK. */
typedef struct rv_state_ops {
    size_t bytes;
    void (*retain)(const void *);
    void (*release)(const void *);
    uint32_t (*snapshot)(const void *, rv_provider *, rv_error *);
} rv_state_ops;
typedef struct rv_state {
    const void *data;
    const rv_state_ops *ops;
    uint64_t cost; /* Declared quota, not measured physical memory. */
} rv_state;

/* Example: propose retains one candidate; provider releases its own reference.
 * Runtime alone revalidates and atomically installs outside provider callbacks. */
typedef struct rv_edit {
    const void *before;
    void *context;
    uint32_t (*propose)(void *, const rv_state *);
} rv_edit;
struct rv_edit_ops {
    size_t bytes;
    uint32_t (*prepare)(void *, uint32_t, const rv_edit *, const rv_args *,
                        rv_pending *, rv_error *);
    uint32_t (*write)(void *, const rv_edit *, const rv_write *,
                      rv_pending *, rv_error *);
};
/* Example: non-byte state leaves byte_edits null instead of exposing writes. */
typedef struct rv_state_init {
    rv_state initial;
    rv_provider implementation;
    const rv_edit_ops *byte_edits;
} rv_state_init;
/* Example: publish retains original uncertain target effects for reconcile. */
typedef struct rv_backing_ops {
    size_t bytes;
    uint32_t (*refresh)(void *, const rv_edit *, const rv_args *,
                        rv_pending *, rv_error *);
    uint32_t (*publish)(void *, const void *, const rv_args *,
                        rv_pending *, rv_error *);
} rv_backing_ops;
/* Example: source attachments and initial state install as one creation. */
typedef struct rv_buffer_init {
    rv_state_init state;
    rv_args attachments;
    const rv_backing_ops *backing;
} rv_buffer_init;
/* Stage exactly one primary; e.g. Parse stages resource, Buffer factory buffer.
 * Auxiliary access remains provisional until the declared result is accepted. */
struct rv_creation {
    uint32_t (*keyed)(void *, rv_bytes key, const rv_provider *);
    void *context;
    uint32_t (*resource)(void *, const rv_provider *);
    uint32_t (*buffer)(void *, const rv_buffer_init *);
    uint32_t (*retained)(void *, const rv_state_init *);
    uint32_t (*auxiliary)(void *, const rv_provider *, rv_access *);
};
struct rv_factory_ops {
    size_t bytes;
    uint32_t (*create)(void *, uint32_t, const rv_creation *, const rv_args *,
                       rv_pending *, rv_error *);
};

/* Example: open stages cursor state with the same Resource identity and zero
 * operation rights. Bind and capture do not invoke open again. */
typedef struct rv_open {
    void *context;
    uint32_t (*stage)(void *, const rv_provider *);
} rv_open;
struct rv_open_ops {
    size_t bytes;
    uint32_t (*begin)(void *, const rv_open *, rv_pending *, rv_error *);
};
/* Example: sequential calls set positional=0 and offset=0. */
typedef struct rv_read {
    uint32_t positional;
    uint64_t offset, maximum;
} rv_read;
struct rv_write {
    uint32_t positional;
    uint64_t offset;
    rv_bytes data;
};
/* Example: positional readonly uses read_modes=2, write_modes=0, write=NULL.
 * Packet declarations are empty exactly when the corresponding callback is
 * absent. Offset/transfer alignment and limits are checked before effects. */
struct rv_io_ops {
    size_t bytes;
    uint32_t read_modes, write_modes;
    uint64_t alignment, transfer_limit;
    rv_bytes receive_type, send_type;
    uint32_t (*read)(void *, const rv_read *, rv_pending *, rv_error *);
    uint32_t (*write)(void *, const rv_write *, rv_pending *, rv_error *);
    uint32_t (*receive)(void *, const rv_args *, rv_pending *, rv_error *);
    uint32_t (*send)(void *, const rv_args *, rv_pending *, rv_error *);
};
/* Example: lookup/list return stable provider keys; materialize revalidates
 * the selected key. Names affect association, never grants or object identity. */
struct rv_namespace_ops {
    size_t bytes;
    /* Required U32 flags: CONTAINER=1, CONTAINER|EMPTY=3; zero activity. */
    uint32_t (*flags)(void *, rv_pending *, rv_error *);
    uint32_t (*lookup)(void *, rv_bytes, rv_pending *, rv_error *);
    uint32_t (*list)(void *, uint64_t, uint64_t, rv_pending *, rv_error *);
    uint32_t (*materialize)(void *, rv_bytes, const rv_creation *,
                            rv_pending *, rv_error *);
    uint32_t (*create)(void *, rv_bytes, const rv_creation *,
                       rv_pending *, rv_error *);
    uint32_t (*remove)(void *, rv_bytes, rv_bytes, rv_pending *, rv_error *);
    /* Same mounted namespace: destination name and retained source key. */
    uint32_t (*link)(void *, rv_bytes, rv_bytes, rv_pending *, rv_error *);
    /* Record(name, key, Choice(Root Unit=0, Key Bytes=1), new_name); no refs. */
    uint32_t (*rename)(void *, const rv_args *, rv_pending *, rv_error *);
};

/* Example: expected is entirely zero when absent, or present=1 with state. */
typedef struct rv_expected {
    uint32_t present;
    rv_identity subject;
    uint64_t revision;
} rv_expected;
/* Example: identical resubmit joins; changed fields under the same reserved
 * request reject Conflict. Input is copied before submission returns. */
typedef struct rv_call {
    uint64_t request;
    uint32_t operation;
    rv_access subject;
    uint32_t command;
    rv_expected expected;
    rv_args input;
} rv_call;
/* Example: reserve -> submit -> result after lost delivery -> adopt -> release.
 * Pending results are delivery views; their finish never drops request custody.
 * Reserved result returns RV_NOT_READY without a view. Caller-view poll accepts
 * an all-zero wake for passive lookup; nonempty wakes require every callback.
 * All callbacks are required; the context stays owned by the supplying host. */
typedef struct rv_caller_ops {
    size_t bytes;
    uint32_t (*reserve)(void *, uint64_t *);
    uint32_t (*submit)(void *, const rv_call *, rv_pending *, rv_error *);
    uint32_t (*result)(void *, uint64_t, rv_pending *, rv_error *);
    uint32_t (*cancel)(void *, uint64_t);
    uint32_t (*reconcile)(void *, uint64_t);
    uint32_t (*adopt)(void *, uint64_t);
    uint32_t (*release)(void *, uint64_t);
} rv_caller_ops;
typedef struct rv_caller { void *context; const rv_caller_ops *ops; } rv_caller;

/* Module entry is passive metadata; construction creates one inert instance. */
typedef struct rv_module_context {
    size_t bytes;
    rv_identity instance;
    uint32_t scope; /* Global=0, Session=1. */
    rv_identity session; /* Zero for Global. */
    rv_identity namespace_resource;
    rv_caller imports;
    rv_registrar exports;
} rv_module_context;
/* Failed alone carries a nonzero failure. All other errors are zero. */
enum { RV_CONSTRUCTED=0, RV_STARTING=1, RV_READY=2,
       RV_CLOSING=3, RV_MODULE_CLOSED=4, RV_MODULE_FAILED=5 };
enum { RV_MORE=0, RV_IDLE=1, RV_STOPPED=2 };
typedef struct rv_module_status {
    uint32_t phase;
    rv_error failure;
} rv_module_status;
/* Required callbacks; all module calls are serialized without unwinding. */
typedef struct rv_module_driver {
    size_t bytes;
    uint32_t (*status)(void *, rv_module_status *);
    uint32_t (*advance)(void *, rv_wake, uint32_t *, rv_error *);
    uint32_t (*close)(void *, rv_error *);
} rv_module_driver;
/* destroy runs once, after Closed and the final dependent owner. */
typedef struct rv_module_instance {
    size_t bytes;
    void *context;
    const rv_module_driver *driver;
    void (*destroy)(void *);
} rv_module_instance;
/* Requested export and complete API; metadata itself grants no access. */
typedef struct rv_module_import {
    rv_bytes name, export_path, declaration;
    uint32_t access, create_access, retain_loans;
} rv_module_import;

/* Exact package dependency: scope Same=0 or Global=1; optional is 0 or 1. */
typedef struct rv_module_dependency {
    uint8_t module[16];
    uint32_t revision;
    uint32_t scope;
    uint32_t optional;
    const rv_module_import *imports;
    size_t import_count;
} rv_module_dependency;
/* Required paths and callable metadata remain valid under the artifact lease.
 * construct borrows context. OK transfers an instance; error leaves output
 * zero and retains no context/effect. Required/export bounds are at most 256. */
typedef struct rv_module {
    size_t bytes;
    rv_version version;
    uint8_t id[16];
    rv_bytes name;
    uint32_t revision;
    uint32_t scopes; /* Global=1, Session=2. */
    const rv_module_dependency *dependencies;
    size_t dependency_count;
    const rv_bytes *required;
    size_t required_count;
    size_t export_limit;
    uint32_t (*construct)(const rv_module_context *, rv_module_instance *,
                          rv_error *);
} rv_module;
/* Example export: const rv_module *rv_module_entry(void). */

#endif
