#ifndef RV_ABI_H
#define RV_ABI_H
#define RV_ABI_VERSION { 0, 1, 0, 0 } /* Until official release. */
#include <stddef.h>
#include <stdint.h>

struct rv_version {
    uint32_t major, minor, patch;
    uint32_t reserved; /* Must be zero. */
};
static inline int rv_abi_compatible(struct rv_version version) {
    const struct rv_version current = RV_ABI_VERSION;
    return version.major == current.major && version.minor == current.minor
        && version.patch == current.patch && version.reserved == 0;
}

struct rv_access;
struct rv_provider;
struct rv_resource_export;
struct rv_pending;
struct rv_state_pending;
struct rv_failure;
struct rv_connection { void *context; const struct rv_access *table; };

struct rv_bytes { const uint8_t *data; size_t len; };
struct rv_contract { struct rv_bytes name; uint32_t version; };
struct rv_request {
    uint32_t method;
    uint64_t subject;
    struct rv_contract contract;
    struct rv_bytes data;
    const uint64_t *resources;
    size_t resource_count;
};
struct rv_reply {
    uint64_t error;
    struct rv_bytes data;
    void *state;
    uint64_t state_bytes;
};
struct rv_module {
    struct rv_version version;
    uint32_t size, kind;
    const void *declaration;
    struct rv_contract create_contract, read_contract, edit_contract;
    void *(*create)(const void *, const struct rv_connection *);
    void (*destroy)(void *);
    struct rv_reply (*call)(void *, const struct rv_request *);
    struct rv_reply (*initialize)(void *, const struct rv_request *);
    struct rv_reply (*prepare)(void *, const void *, const struct rv_request *);
    /* Async slots supersede finite callbacks. Copy request views on entry;
     * old state and context stay borrowed until state_pending.release. */
    struct rv_state_pending (*initialize_async)(void *, const struct rv_request *);
    struct rv_state_pending (*prepare_async)(void *, const void *, const struct rv_request *);
    struct rv_reply (*observe)(void *, const void *, const struct rv_request *);
    struct rv_pending (*inspect)(void *, const void *, const struct rv_request *);
    void (*release)(void *, struct rv_reply *);
    void (*drop_state)(void *, void *);
    uint64_t (*unload)(void *);
    size_t exports;
    struct rv_failure (*describe)(const void *, size_t, struct rv_resource_export *);
    struct rv_pending (*request)(void *, size_t, const struct rv_request *);
    /* Passive catalog identity and requirements; no construction required. */
    struct rv_contract identity;
    const struct rv_contract *dependencies;
    size_t dependency_count;
};

/* Experimental module ABI 0.1.0. Kind: Resource=1, Model=2.
 * Methods: read=1, write=2.
 * Inputs are borrowed during a call. Copy anything needed afterwards.
 * Output bytes live until release. A successful state transfers to the host:
 * release sees state=NULL; drop_state eventually destroys the transferred state.
 * Resource request and optional Model inspect return owned pending results.
 * Model initialize/prepare/observe settle synchronously. No callback may unwind.
 * Model observe receives read_contract with no resource slots. Empty data
 * requests the finite acceptance descriptor; other data is model-defined.
 * Each read uses the exact immutable state, including retained older states.
 * Optional Model inspect returns Pending for deferred state reads. The host
 * pins the state through Pending.release. Its input is borrowed only for the
 * callback. Success uses read_contract and no references; failures are model
 * error codes, with no external-effect recovery outcome in this profile.
 * Unload happens only after contexts, calls, results and states have drained.
 */
/* create receives optional borrowed caller access: retain before storing.
 * Factory state is inert: metadata lookup is allowed, starting work is not.
 * unload returns 0 only after all external callbacks/work have drained;
 * readiness always leaves the provider usable: another grouped member may
 * return busy (8). destroy follows the owner's successful removal of the set.
 */
const struct rv_module *rv_module_entry(void);

/* Caller ABI. Views are borrowed unless a release callback is supplied. */
struct rv_failure { uint64_t code, resource; };
struct rv_entry { struct rv_bytes name; uint64_t resource; };
struct rv_listing {
    const struct rv_entry *entries;
    size_t count;
    void *owner;
    void (*release)(void *);
};
struct rv_wake {
    const void *context;
    void (*retain)(const void *);
    void (*release)(const void *);
    void (*wake)(const void *);
};
struct rv_outcome {
    uint32_t status; /* 0 pending, 1 success, 2 rejected, 3 failed, 4 unknown */
    struct rv_failure error;
    struct rv_contract contract;
    struct rv_bytes data;
    const uint64_t *resources;
    size_t resource_count;
    struct rv_bytes recovery;
};
struct rv_pending {
    void *context;
    struct rv_outcome (*poll)(void *, struct rv_wake);
    void (*release)(void *);
};
struct rv_state_poll { uint32_t status; uint64_t error; };
struct rv_state_pending {
    void *context;
    struct rv_state_poll (*poll)(void *, struct rv_wake);
    struct rv_reply (*take)(void *);
    void (*release)(void *);
};
/* State poll: 0 pending, 1 ready, 2 failed. Poll never transfers state.
 * After ready, take transfers exactly one owned Reply state; the Model's
 * release/drop_state callbacks apply. Pending release destroys untaken state.
 */
struct rv_access {
    struct rv_version version;
    uint32_t size;
    void (*retain)(void *);
    void (*release)(void *);
    struct rv_failure (*root)(void *, uint64_t *);
    struct rv_failure (*resolve)(void *, uint64_t, struct rv_bytes, uint64_t *);
    struct rv_failure (*children)(void *, uint64_t, struct rv_listing *);
    struct rv_pending (*request)(void *, const struct rv_request *);
    struct rv_failure (*close)(void *, uint64_t);
    struct rv_failure (*reconnect)(void *, struct rv_connection *);
    struct rv_failure (*withdraw)(void *, uint64_t);
    struct rv_failure (*publish)(void *, uint64_t, struct rv_bytes,
        const struct rv_provider *, uint64_t *);
    struct rv_failure (*capture)(void *, uint64_t, uint32_t,
        struct rv_connection *);
};
/* Connection/Pending own one reference, released through their own table.
 * capture retains selected existing rights in this namespace through its
 * returned Connection; release does not close the shared Resource.
 * Access is single-threaded. Wake retain/release/wake must be thread-safe;
 * poll borrows Wake, so retaining it requires retain. request copies inputs.
 * Poll observes results only: the implementation owner drives execution.
 * Terminal Outcome views live until Pending.release. Listing owns its views
 * until Listing.release. Failed metadata calls leave output pointers untouched.
 * Every callback must obey the no-unwind rule. resource=0 means no error subject.
 */
/* Resource exports share one inert context. Empty path
 * denotes the chosen mount. Surface methods and slots use read=1/write=2 bits.
 * describe returns immutable views valid while the library remains loaded.
 * Runtime invokes call and polls Pending on one thread outside registry guards.
 * Provider polling advances its own work and can await imported Kernel calls.
 * call copies borrowed input. Release each Pending before destroying context.
 * unload returns busy while imports, wakes or retained work need native code.
 */
struct rv_surface {
    struct rv_contract contract;
    uint32_t methods;
    const uint32_t *input;
    size_t input_count;
    const uint32_t *output;
    size_t output_count;
};
struct rv_resource_export {
    struct rv_bytes path;
    const struct rv_surface *surfaces;
    size_t surface_count;
    uint32_t forwarding; /* 0 fixed slots; 1 complete Messages and grants. */
};

/* A borrowed provider descriptor. Success retains; rejection retains nothing. */
struct rv_provider_ops {
    struct rv_version version;
    uint32_t size;
    void (*retain)(void *);
    void (*release)(void *);
    struct rv_pending (*request)(void *, const struct rv_request *);
    /* Owned access: fence delivery and wake pending calls; cannot fail. */
    struct rv_failure (*close)(void *);
    struct rv_failure (*unload)(void *);
};
/* An owned provider close fences operations before cleanup. Failure retains
 * context and code for a retry; successful cleanup is not invoked again.
 * The callback must tolerate its own partial cleanup from earlier attempts.
 */
struct rv_provider {
    void *context;
    const struct rv_provider_ops *table;
    const struct rv_surface *surfaces;
    size_t surface_count;
    uint32_t forwarding;
    uint32_t owned;
};

#endif
