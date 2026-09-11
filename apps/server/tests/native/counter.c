#include "reovim.h"
#include <stdlib.h>

static void *create(const ReovimConnection *kernel) {
    (void)kernel;
    return calloc(8, 1); }
static void destroy(void *context) { free(context); }
static void release(void *context, ReovimReply *reply) {
    (void)context;
    (void)reply; /* Result borrows the context until this release. */
}
static ReovimReply call(void *context, const ReovimRequest *request) {
    uint8_t *value = context;
    if (request->method == 2) {
        if (request->data.len != 1)
            return (ReovimReply){ .error = 3 };
        value[0] += request->data.data[0];
    }
    return (ReovimReply){ .data = { value, 8 } };
}
#define CONTRACT { { (const uint8_t *)"native.counter", 14 }, 1 }
static uint64_t unload(void *context) { (void)context; return 0; }
const ReovimModule *reovim_module_v2(void) {
    static const ReovimModule module = {
        .version = 2, .size = sizeof(ReovimModule), .kind = 1,
        .read_contract = CONTRACT, .edit_contract = CONTRACT,
        .create = create, .destroy = destroy, .unload = unload, .call = call, .release = release,
    };
    return &module;
}
