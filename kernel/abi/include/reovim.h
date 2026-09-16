/* reovim.h - the Driver contract in C. Seven slots a Driver fills, seven
 * calls it may make, one descriptor per Module. Documentation/Design/00-core.md
 * is the contract; this header follows it and never grows a slot. */
#ifndef REOVIM_H
#define REOVIM_H

#include <stddef.h>
#include <stdint.h>

#define RV_MAGIC 0x314d5652u /* "RVM1" little endian */
#define RV_ABI 1u

typedef uint64_t rv_rd;
typedef uint64_t rv_handle;
typedef uint64_t rv_token;
typedef uint64_t rv_hash;

/* Borrowed bytes; a NULL ptr with len 0 is none. */
typedef struct rv_bytes {
    const uint8_t *ptr;
    size_t len;
} rv_bytes;

/* How every slot and call answers. status 0 is ok and value is the
 * rd, handle, count or token the slot answers with; 1..9 are the nine
 * causes in 00-core.md order; 10 is pending and value is the token. */
enum {
    RV_OK = 0, RV_DENIED = 1, RV_UNSUPPORTED = 2, RV_INVALID = 3,
    RV_STALE = 4, RV_BUSY = 5, RV_NOSPACE = 6, RV_CLOSED = 7,
    RV_GONE = 8, RV_CANCELLED = 9, RV_PENDING = 10
};
typedef struct rv_reply {
    uint32_t status;
    uint64_t value;
} rv_reply;

/* Flags and rights bits; event kinds for changed. */
enum { RV_READ = 1, RV_WRITE = 2, RV_CONTAINER = 4 };
enum { RV_MODIFY = 0, RV_ATTRIB = 1 };

/* The seven calls, valid during the slot call that handed them over.
 * name NULL means none. apis are canonical declaration bytes. done with
 * status 0 delivers out as the bytes of a read or call, or its len as
 * the count of a write (ptr may be NULL). submit takes a request and
 * answers a reply in the wire encoding with sequence 0; the reply bytes
 * live until the next submit on the same rv_calls. */
typedef struct rv_calls {
    void *ctx;
    rv_reply (*register_)(void *ctx, uint32_t driver, rv_rd parent,
                          const char *name);
    rv_reply (*name)(void *ctx, rv_rd rd, const char *name);
    rv_reply (*report)(void *ctx, rv_rd rd, uint32_t flags,
                       const rv_bytes *apis, size_t count);
    rv_reply (*unregister)(void *ctx, rv_rd rd);
    rv_reply (*changed)(void *ctx, rv_rd rd, uint32_t kind);
    rv_reply (*done)(void *ctx, rv_token token, uint32_t status,
                     rv_bytes out);
    rv_reply (*open)(void *ctx, rv_rd rd, uint32_t rights);
    rv_reply (*submit)(void *ctx, rv_bytes op, rv_bytes *reply);
} rv_calls;

/* The seven slots. A NULL slot answers RV_UNSUPPORTED. read fills up to
 * len bytes of buf and answers the count; 0 is end of content. call's
 * out points at Driver-owned bytes that live until the next slot call
 * on this Driver. Only read, write and call may answer RV_PENDING. */
typedef struct rv_driver {
    void *ctx;
    rv_reply (*lookup)(void *ctx, const rv_calls *k, rv_rd rd,
                       const char *name);
    rv_reply (*open)(void *ctx, const rv_calls *k, rv_rd rd,
                     uint32_t rights);
    rv_reply (*read)(void *ctx, const rv_calls *k, uint64_t h, uint64_t at,
                     uint8_t *buf, size_t len);
    rv_reply (*write)(void *ctx, const rv_calls *k, uint64_t h, uint64_t at,
                      rv_bytes buf);
    rv_reply (*call)(void *ctx, const rv_calls *k, uint64_t h, rv_hash api,
                     uint16_t method, rv_bytes in, rv_bytes *out);
    rv_reply (*watch)(void *ctx, const rv_calls *k, rv_rd rd, uint32_t on);
    rv_reply (*close)(void *ctx, const rv_calls *k, uint64_t h);
} rv_driver;

/* One Module: checked by magic and abi before anything is called.
 * drivers[i] is driver id i. stop may be NULL. */
typedef struct rv_module {
    uint32_t magic;
    uint32_t abi;
    const char *name;
    void *ctx;
    rv_reply (*start)(void *ctx, const rv_calls *k, rv_rd home);
    void (*stop)(void *ctx);
    const rv_driver *drivers;
    size_t driver_count;
} rv_module;

/* The one symbol a .rvm exports. */
const rv_module *rv_module_entry(void);

#endif
