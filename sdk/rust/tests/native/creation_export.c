#include "pending.c"

typedef struct { rv_provider primary, auxiliary; } Created;
static uint64_t staging_calls;
static int reject_stage;
static uint32_t stage_primary(void *context, const rv_provider *provider) {
    staging_calls++;
    if (reject_stage) return RV_DENIED;
    ((Created *)context)->primary = *provider;
    return RV_OK;
}
static uint32_t stage_auxiliary(void *context, const rv_provider *provider,
                                uint64_t *token) {
    staging_calls++;
    ((Created *)context)->auxiliary = *provider;
    *token = 0x44;
    return RV_OK;
}
static void read_and_destroy(rv_provider *provider, uint64_t expected) {
    const rv_interface *api = provider->interfaces;
    rv_args input = {0};
    rv_pending pending = {0};
    rv_error error = {0};
    if (api->control->ioctl(api->context, 1, &input, &pending, &error)) abort();
    if (fixture_observe(&pending, 0, 0) != expected) abort();
    if (pending.ops->finish(pending.context)) abort();
    if (provider->stop(provider->context)) abort();
    provider->destroy(provider->context);
}
void fixture_factory(rv_provider *provider, uint32_t rejected) {
    reject_stage = rejected == 2;
    Created created = {0};
    rv_creation creation = {.context = &created, .resource = stage_primary,
        .auxiliary = stage_auxiliary};
    const rv_interface *api = provider->interfaces;
    if (api->exec || api->control || !api->factory) abort();
    rv_args input = {0};
    rv_pending pending = {0};
    rv_error error = {0};
    uint32_t result = api->factory->create(api->context, 1, &creation,
                                          &input, &pending, &error);
    if (rejected) {
        if (result != RV_DENIED || error.code != RV_DENIED ||
            pending.context || pending.ops || created.primary.context) abort();
        return;
    }
    if (result || fixture_observe(&pending, 0, 0) != 17) abort();
    if (pending.ops->finish(pending.context)) abort();
    read_and_destroy(&created.primary, 91);
    read_and_destroy(&created.auxiliary, 92);
}
uint64_t fixture_staging_calls(void) { return staging_calls; }
