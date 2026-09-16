#include "operations.c"

uint32_t fixture_stage(const rv_registrar *registrar, rv_bytes path,
                       const rv_provider *provider) {
    if (!registrar || registrar->bytes != sizeof(*registrar) ||
        !registrar->stage) {
        return 1;
    }
    return registrar->stage(registrar->context, path, provider);
}
