#include "provider_export.c"
#include <string.h>

typedef struct { rv_provider accepted; } Registry;
static uint64_t calls;
static uint32_t registry_stage(void *context, rv_bytes path,
                                const rv_provider *provider) {
    calls++;
    Registry *registry = context;
    if (path.len != 6 || memcmp(path.ptr, "device", 6)) return RV_DENIED;
    if (registry->accepted.context) return RV_CONFLICT;
    registry->accepted = *provider;
    return RV_OK;
}
uint32_t fixture_registry_create(rv_registrar *output) {
    Registry *registry = calloc(1, sizeof(*registry));
    if (!registry) return RV_PRESSURE;
    *output = (rv_registrar){sizeof(*output), registry, registry_stage};
    return RV_OK;
}
uint64_t fixture_registry_read(const rv_registrar *registrar) {
    Registry *registry = registrar->context;
    return fixture_control(&registry->accepted, 1);
}
void fixture_registry_destroy(const rv_registrar *registrar) {
    Registry *registry = registrar->context;
    if (registry->accepted.context) {
        rv_provider *provider = &registry->accepted;
        if (provider->stop(provider->context)) abort();
        provider->destroy(provider->context);
    }
    free(registry);
}
uint64_t fixture_registry_calls(void) { return calls; }
