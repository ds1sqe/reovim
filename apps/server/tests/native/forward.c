#include "reovim.h"
#include <assert.h>
#include <stdlib.h>
#include <stdatomic.h>
#include <string.h>

typedef struct {
    ReovimConnection kernel;
    uint64_t counter;
    ReovimPending pending;
    uint8_t result[8];
    _Atomic size_t wakes;
} Context;
static void retain_wake(const void *pointer) {
    Context *context = (Context *)pointer;
    atomic_fetch_add(&context->wakes, 1);
}
static void release_wake(const void *pointer) {
    Context *context = (Context *)pointer;
    atomic_fetch_sub(&context->wakes, 1);
}
static void noop(const void *context) { (void)context; }
static uint64_t collect(Context *context) {
    if (!context->pending.context) return 0;
    ReovimWake wake = { .context = context, .retain = retain_wake,
                       .release = release_wake, .wake = noop };
    ReovimOutcome outcome = context->pending.poll(context->pending.context, wake);
    if (!outcome.status) return 8;
    uint64_t error = outcome.error.code;
    if (outcome.status == 1 && outcome.data.len == 8)
        memcpy(context->result, outcome.data.data, 8);
    else if (outcome.status == 1) error = 11;
    context->pending.release(context->pending.context);
    context->pending.context = NULL;
    return error;
}
static void *create(const ReovimConnection *kernel) {
    if (!kernel || kernel->table->version != 1) return NULL;
    Context *context = calloc(1, sizeof(Context));
    if (!context) return NULL;
    atomic_init(&context->wakes, 0);
    uint64_t root;
    ReovimBytes path = { (const uint8_t *)"counter", 7 };
    if (kernel->table->root(kernel->context, &root).code ||
        kernel->table->resolve(kernel->context, root, path,
                               &context->counter).code) {
        free(context);
        return NULL;
    }
    context->kernel = *kernel;
    kernel->table->retain(kernel->context);
    return context;
}
static void destroy(void *pointer) {
    Context *context = pointer;
    assert(!context->pending.context);
    assert(atomic_load(&context->wakes) == 0);
    context->kernel.table->release(context->kernel.context);
    free(context);
}
static uint64_t unload(void *pointer) {
    Context *context = pointer;
    uint64_t result = collect(context);
    if (context->pending.context) return result;
    return atomic_load(&context->wakes) ? 8 : 0;
}
static ReovimReply call(void *pointer, const ReovimRequest *input) {
    Context *context = pointer;
    if (input->method == 2) {
        if (context->pending.context) return (ReovimReply){ .error = 8 };
        ReovimRequest request = {
            .method = 2, .subject = context->counter,
            .contract = { { (const uint8_t *)"external.counter", 16 }, 1 },
            .data = input->data,
        };
        context->pending = context->kernel.table->request(
            context->kernel.context, &request);
        return (ReovimReply){ 0 }; /* Queued, not yet applied. */
    }
    uint64_t error = collect(context);
    if (error) return (ReovimReply){ .error = error };
    return (ReovimReply){ .data = { context->result, 8 } };
}
static void release(void *context, ReovimReply *reply) {
    (void)context;
    (void)reply;
}
#define CONTRACT { { (const uint8_t *)"native.forward", 14 }, 1 }
const ReovimModule *reovim_module_v2(void) {
    static const ReovimModule module = {
        .version = 2, .size = sizeof(ReovimModule), .kind = 1,
        .read_contract = CONTRACT, .edit_contract = CONTRACT,
        .create = create, .destroy = destroy, .unload = unload,
        .call = call, .release = release,
    };
    return &module;
}
