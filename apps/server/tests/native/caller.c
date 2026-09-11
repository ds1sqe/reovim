#include "reovim.h"

int reovim_client_start(ReovimConnection connection, ReovimPending *pending) {
    const ReovimAccess *api = connection.table;
    if (api->version != 1 || api->size < sizeof(ReovimAccess)) return 1;
    uint64_t root, counter;
    if (api->root(connection.context, &root).code) return 2;
    ReovimBytes path = { (const uint8_t *)"counter", 7 };
    if (api->resolve(connection.context, root, path, &counter).code) return 3;
    const uint8_t increment[] = { 11, 0, 0, 0, 0, 0, 0, 0 };
    ReovimRequest request = {
        .method = 2, .subject = counter,
        .contract = { { (const uint8_t *)"external.counter", 16 }, 1 },
        .data = { increment, sizeof(increment) },
    };
    *pending = api->request(connection.context, &request);
    return 0;
}

/* Copy the semantic result before releasing the call's borrowed views. */
int reovim_client_finish(ReovimPending pending, uint64_t *value) {
    ReovimWake wake = { 0 };
    /* This function is called only after Runtime settles the request. */
    extern void reovim_noop(const void *);
    wake.retain = reovim_noop;
    wake.release = reovim_noop;
    wake.wake = reovim_noop;
    ReovimOutcome result = pending.poll(pending.context, wake);
    if (result.status != 1 || result.data.len != 8) return 1;
    uint64_t decoded = 0;
    for (unsigned i = 0; i < 8; ++i)
        decoded |= (uint64_t)result.data.data[i] << (i * 8);
    *value = decoded;
    pending.release(pending.context);
    return 0;
}
void reovim_noop(const void *context) { (void)context; }
