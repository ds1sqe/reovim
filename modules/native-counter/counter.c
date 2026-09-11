#include "reovim.h"
#include <stdlib.h>

static void *create(const void *declaration,
                       const struct rv_connection *kernel) {
    (void)declaration;
    (void)kernel;
    return calloc(8, 1); }
static void destroy(void *context) { free(context); }
static void release(void *context, struct rv_reply *reply) {
    (void)context;
    (void)reply; /* Result borrows the context until this release. */
}
static struct rv_reply call(void *context, const struct rv_request *request) {
    uint8_t *value = context;
    if (request->method == 2) {
        if (request->data.len != 1)
            return (struct rv_reply){ .error = 3 };
        value[0] += request->data.data[0];
    }
    return (struct rv_reply){ .data = { value, 8 } };
}
#define CONTRACT { { (const uint8_t *)"native.counter", 14 }, 1 }
static uint64_t unload(void *context) { (void)context; return 0; }
const struct rv_module *rv_module_entry(void) {
    static const struct rv_module module = {
        .version = RV_ABI_VERSION, .size = sizeof(struct rv_module), .kind = 1,
        .read_contract = CONTRACT, .edit_contract = CONTRACT,
        .create = create, .destroy = destroy, .unload = unload, .call = call, .release = release,
    };
    return &module;
}
