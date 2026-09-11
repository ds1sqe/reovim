#include "reovim.h"
#include <stdlib.h>

#define CONTRACT(name) { { (const uint8_t *)(name), sizeof(name) - 1 }, 1 }
#ifndef MODULE_NAME
#define MODULE_NAME "example.feature"
#endif
#ifndef REQUIRED_FIRST
#define REQUIRED_FIRST "example.storage"
#endif
static uint64_t constructions;
static void *create(const void *declaration,
                    const struct rv_connection *kernel) {
    (void)declaration;
    (void)kernel;
#ifdef REQUIRE_STORAGE
    uint64_t root, dependency;
    const struct rv_bytes path = {
        (const uint8_t *)"services/storage", 16
    };
    if (!kernel || kernel->table->root(kernel->context, &root).code ||
        kernel->table->resolve(kernel->context, root, path, &dependency).code)
        return NULL;
#endif
    uint64_t *value = malloc(sizeof(*value));
    if (value) *value = ++constructions;
    return value;
}
static void destroy(void *context) { free(context); }
static uint64_t unload(void *context) { (void)context; return 0; }
static void release(void *context, struct rv_reply *reply) {
    (void)context;
    (void)reply;
}
static struct rv_reply call(void *context, const struct rv_request *request) {
    (void)request;
    return (struct rv_reply){ .data = { context, sizeof(uint64_t) } };
}
const struct rv_module *rv_module_entry(void) {
#ifndef LEAF
    static const struct rv_contract dependencies[] = {
        CONTRACT(REQUIRED_FIRST), CONTRACT("example.index")
    };
#endif
    static const struct rv_module module = {
        .version = RV_ABI_VERSION, .size = sizeof(struct rv_module), .kind = 1,
        .identity = CONTRACT(MODULE_NAME),
#ifndef LEAF
        .dependencies = dependencies, .dependency_count = 2,
#endif
        .read_contract = CONTRACT("example.constructions"),
        .edit_contract = CONTRACT("example.constructions"),
        .create = create, .destroy = destroy, .unload = unload,
        .call = call, .release = release,
    };
    return &module;
}
