#ifndef REOVIM_ABI_H
#define REOVIM_ABI_H
#include <stddef.h>
#include <stdint.h>

typedef struct ReovimAccess ReovimAccess;
typedef struct { void *context; const ReovimAccess *table; } ReovimConnection;

typedef struct { const uint8_t *data; size_t len; } ReovimBytes;
typedef struct { ReovimBytes name; uint32_t version; } ReovimContract;
typedef struct {
    uint32_t method;
    uint64_t subject;
    ReovimContract contract;
    ReovimBytes data;
    const uint64_t *resources;
    size_t resource_count;
} ReovimRequest;
typedef struct {
    uint64_t error;
    ReovimBytes data;
    void *state;
    uint64_t state_bytes;
} ReovimReply;
typedef struct {
    uint32_t version, size, kind;
    ReovimContract create_contract, read_contract, edit_contract;
    void *(*create)(const ReovimConnection *);
    void (*destroy)(void *);
    ReovimReply (*call)(void *, const ReovimRequest *);
    ReovimReply (*initialize)(void *, const ReovimRequest *);
    ReovimReply (*prepare)(void *, const void *, const ReovimRequest *);
    ReovimReply (*observe)(void *, const void *);
    void (*release)(void *, ReovimReply *);
    void (*drop_state)(void *, void *);
    uint64_t (*unload)(void *);
} ReovimModule;

/* Module version 2; kind 1 = Resource, 2 = model; method 1 = read, 2 = write.
 * Inputs are borrowed during a call. Copy anything needed afterwards.
 * Output bytes live until release. A successful state transfers to the host:
 * release sees state=NULL; drop_state eventually destroys the transferred state.
 * All callbacks settle synchronously and must not unwind across the boundary.
 * Unload happens only after contexts, calls, results and states have drained.
 */
/* create receives optional borrowed caller-v1 access: retain before storing.
 * Factory state is inert: metadata lookup is allowed, starting work is not.
 * unload returns 0 only after all external callbacks/work have drained;
 * readiness always leaves the provider usable: another grouped member may
 * return busy (8). destroy follows the owner's successful removal of the set.
 */
const ReovimModule *reovim_module_v2(void);

/* Caller ABI. Views are borrowed unless a release callback is supplied. */
typedef struct { uint64_t code, resource; } ReovimFailure;
typedef struct { ReovimBytes name; uint64_t resource; } ReovimEntry;
typedef struct {
    const ReovimEntry *entries;
    size_t count;
    void *owner;
    void (*release)(void *);
} ReovimListing;
typedef struct {
    const void *context;
    void (*retain)(const void *);
    void (*release)(const void *);
    void (*wake)(const void *);
} ReovimWake;
typedef struct {
    uint32_t status; /* 0 pending, 1 success, 2 rejected, 3 failed, 4 unknown */
    ReovimFailure error;
    ReovimContract contract;
    ReovimBytes data;
    const uint64_t *resources;
    size_t resource_count;
    ReovimBytes recovery;
} ReovimOutcome;
typedef struct {
    void *context;
    ReovimOutcome (*poll)(void *, ReovimWake);
    void (*release)(void *);
} ReovimPending;
struct ReovimAccess {
    uint32_t version, size;
    void (*retain)(void *);
    void (*release)(void *);
    ReovimFailure (*root)(void *, uint64_t *);
    ReovimFailure (*resolve)(void *, uint64_t, ReovimBytes, uint64_t *);
    ReovimFailure (*children)(void *, uint64_t, ReovimListing *);
    ReovimPending (*request)(void *, const ReovimRequest *);
    ReovimFailure (*close)(void *, uint64_t);
    ReovimFailure (*reconnect)(void *, ReovimConnection *);
};
/* Connection/Pending own one reference, released through their own table.
 * Access is single-threaded. Wake retain/release/wake must be thread-safe;
 * poll borrows Wake, so retaining it requires retain. request copies inputs.
 * Poll observes results only: the implementation owner drives execution.
 * Terminal Outcome views live until Pending.release. Listing owns its views
 * until Listing.release. Failed metadata calls leave output pointers untouched.
 * Every callback must obey the no-unwind rule. resource=0 means no error subject.
 */
#endif
