#ifndef RV_TEST_NAMESPACE_SOURCE
#define RV_TEST_NAMESPACE_SOURCE "namespace.c"
#endif
#include RV_TEST_NAMESPACE_SOURCE

typedef struct { uint32_t phase; rv_registrar exports; } Mount;
static uint32_t mount_status(void *context, rv_module_status *output) {
    *output = (rv_module_status){.phase = ((Mount *)context)->phase};
    return RV_OK;
}
static uint32_t mount_advance(void *context, rv_wake wake,
    uint32_t *progress, rv_error *error) {
    (void)wake;
    Mount *mount = context;
    if (mount->phase == RV_CONSTRUCTED) {
        mount->phase = RV_STARTING;
        *progress = RV_MORE;
        return RV_OK;
    }
    if (mount->phase != RV_STARTING) { *progress = RV_IDLE; return RV_OK; }
    rv_provider provider = {0};
    uint32_t result = fixture_directory(&provider);
    if (!result) result = mount->exports.stage(mount->exports.context,
        (rv_bytes){(const uint8_t *)"directory", 9}, &provider);
    if (result) {
        if (provider.context) {
            provider.stop(provider.context);
            provider.destroy(provider.context);
        }
        error->code = result;
        return result;
    }
    mount->phase = RV_READY;
    *progress = RV_MORE;
    return RV_OK;
}
static uint32_t mount_close(void *context, rv_error *error) {
    (void)error;
    ((Mount *)context)->phase = RV_MODULE_CLOSED;
    return RV_OK;
}
static void mount_destroy(void *context) { free(context); }
static const rv_module_driver mount_driver = {
    sizeof(rv_module_driver), mount_status, mount_advance, mount_close
};
static uint32_t mount_construct(const rv_module_context *context,
    rv_module_instance *output, rv_error *error) {
    Mount *mount = calloc(1, sizeof(*mount));
    if (!mount) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    mount->exports = context->exports;
    *output = (rv_module_instance){sizeof(*output), mount,
        &mount_driver, mount_destroy};
    return RV_OK;
}
static const rv_bytes mount_required[] = {
    {(const uint8_t *)"directory", 9}
};
static const rv_module mount_descriptor = {
    .bytes = sizeof(rv_module), .version = RV_ABI_VERSION,
    .id = {0xb3}, .name = {(const uint8_t *)"namespace-fixture", 17},
    .revision = 1, .scopes = 1, .required = mount_required,
    .required_count = 1, .export_limit = 1, .construct = mount_construct
};
const rv_module *rv_module_entry(void) { return &mount_descriptor; }
