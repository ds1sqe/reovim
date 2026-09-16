#include "reovim.h"

int reovim_client_start(struct rv_connection connection, struct rv_pending *pending) {
    const struct rv_access *api = connection.table;
    if (!rv_abi_compatible(api->version) || api->size < sizeof(struct rv_access)) return 1;
    uint64_t root, counter;
    if (api->root(connection.context, &root).code) return 2;
    struct rv_bytes path = { (const uint8_t *)"counter", 7 };
    if (api->resolve(connection.context, root, path, &counter).code) return 3;
    uint32_t flags = 0;
    if (!api->flags || api->flags(connection.context, root, &flags).code) return 5;
    if (flags != RV_RESOURCE_CONTAINER) return 6;
    if (api->flags(connection.context, counter, &flags).code) return 7;
    if (flags != (RV_RESOURCE_CONTAINER | RV_RESOURCE_EMPTY)) return 8;
    const uint8_t increment[] = { 11, 0, 0, 0, 0, 0, 0, 0 };
    struct rv_request request = {
        .method = 2, .subject = counter,
        .contract = { { (const uint8_t *)"external.counter", 16 }, 1 },
        .data = { increment, sizeof(increment) },
    };
    struct rv_connection captured;
    if (api->capture(connection.context, counter, 2, &captured).code) return 4;
    *pending = captured.table->request(captured.context, &request);
    /* Admitted work retains the selected capture after the caller releases it. */
    captured.table->release(captured.context);
    return 0;
}

/* Copy the semantic result before releasing the call's borrowed views. */
int reovim_client_finish(struct rv_pending pending, uint64_t *value) {
    struct rv_wake wake = { 0 };
    /* This function is called only after Runtime settles the request. */
    extern void reovim_noop(const void *);
    wake.retain = reovim_noop;
    wake.release = reovim_noop;
    wake.wake = reovim_noop;
    struct rv_outcome result = pending.poll(pending.context, wake);
    if (result.status != 1 || result.data.len != 8) return 1;
    uint64_t decoded = 0;
    for (unsigned i = 0; i < 8; ++i)
        decoded |= (uint64_t)result.data.data[i] << (i * 8);
    *value = decoded;
    pending.release(pending.context);
    return 0;
}
void reovim_noop(const void *context) { (void)context; }
