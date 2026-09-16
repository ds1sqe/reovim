#include "operations.c"

typedef struct ModuleData ModuleData;
typedef struct { Counter counter; ModuleData *module; } Embedded;
struct ModuleData {
    uint64_t magic, staged, retired;
    uint32_t phase;
    rv_registrar exports;
    Embedded devices[2];
};
static uint64_t constructed, module_destroyed, late_closed;
static uint32_t mode;
static int close_ready = 1;
static const uint64_t live_magic = UINT64_C(0xa1735ef291b04c26);

static void embedded_destroy(void *context) {
    Embedded *device = context;
    if (device->module->magic != live_magic) abort();
    device->module->retired++;
    destroyed++;
}
static rv_provider embedded(ModuleData *module, unsigned index) {
    Counter *counter = &module->devices[index].counter;
    counter->interface = interfaces[0];
    counter->interface.context = &module->devices[index];
    return (rv_provider){
        .bytes = sizeof(rv_provider), .context = &module->devices[index],
        .interfaces = &counter->interface, .interface_count = 1,
        .stop = stop, .destroy = embedded_destroy
    };
}
static uint32_t module_status(void *context, rv_module_status *output) {
    ModuleData *module = context;
    *output = (rv_module_status){ .phase = module->phase };
    return RV_OK;
}
static uint32_t module_advance(void *context, rv_wake wake,
                               uint32_t *progress, rv_error *error) {
    ModuleData *module = context;
    wake.retain(wake.context);
    wake.wake(wake.context);
    wake.release(wake.context);
    if (module->phase == RV_CLOSING) {
        if (close_ready) module->phase = RV_MODULE_CLOSED;
        *progress = close_ready && mode != 3 ? RV_STOPPED : RV_IDLE;
        return RV_OK;
    }
    if (module->phase == RV_READY) {
        *progress = RV_IDLE;
        return RV_OK;
    }
    if (module->phase == RV_STARTING && mode == 1) {
        module->phase = RV_READY;
        *progress = RV_IDLE;
        return RV_OK;
    }
    unsigned index = module->phase == RV_CONSTRUCTED ? 0 : 1;
    const uint8_t *name = (const uint8_t *)(index ? "second" : "first");
    rv_bytes path = {name, index ? 6 : 5};
    rv_provider provider = embedded(module, index);
    uint32_t result = module->exports.stage(module->exports.context, path,
                                             &provider);
    if (result != RV_OK) {
        *error = (rv_error){ .code = result };
        return result;
    }
    module->staged++;
    module->phase = index ? RV_READY : RV_STARTING;
    *progress = index ? RV_IDLE : RV_MORE;
    return RV_OK;
}
static uint32_t module_close(void *context, rv_error *error) {
    ModuleData *module = context;
    int ready = close_ready && mode != 3;
    module->phase = ready ? RV_MODULE_CLOSED : RV_CLOSING;
    if (!ready) *error = (rv_error){ .code = RV_BUSY };
    return ready ? RV_OK : RV_BUSY;
}
static void module_destroy(void *context) {
    ModuleData *module = context;
    if (module->magic != live_magic || module->retired != module->staged)
        abort();
    rv_provider late = embedded(module, 0);
    rv_bytes path = {(const uint8_t *)"late", 4};
    if (module->exports.stage(module->exports.context, path, &late) != RV_CLOSED)
        abort();
    late_closed++;
    module->magic = 0;
    module_destroyed++;
    free(module);
}
static const rv_module_driver module_driver = {
    sizeof(rv_module_driver), module_status, module_advance, module_close
};
static uint32_t module_construct(const rv_module_context *context,
                                 rv_module_instance *output, rv_error *error) {
    if (mode == 2) {
        *error = (rv_error){ .code = RV_DENIED };
        return RV_DENIED;
    }
    if (context->bytes != sizeof(*context) || context->scope != 0 ||
        context->session.serial || !context->instance.serial ||
        !context->namespace_resource.serial || !context->imports.ops) {
        *error = (rv_error){ .code = RV_INVALID };
        return RV_INVALID;
    }
    ModuleData *module = calloc(1, sizeof(*module));
    if (!module) {
        *error = (rv_error){ .code = RV_PRESSURE };
        return RV_PRESSURE;
    }
    module->magic = live_magic;
    module->exports = context->exports;
    for (unsigned i = 0; i < 2; i++) {
        module->devices[i].counter.value = 81 + i;
        module->devices[i].module = module;
    }
    *output = (rv_module_instance){
        sizeof(*output), module, &module_driver, module_destroy
    };
    constructed++;
    return RV_OK;
}
static const rv_bytes required_exports[] = {
    {(const uint8_t *)"first", 5}, {(const uint8_t *)"second", 6}
};
static const rv_module descriptor = {
    .bytes = sizeof(rv_module), .version = RV_ABI_VERSION,
    .id = {0xa3}, .name = {(const uint8_t *)"fixture", 7},
    .revision = 1, .scopes = 1,
    .required = required_exports, .required_count = 2, .export_limit = 2,
    .construct = module_construct
};
const rv_module *rv_module_entry(void) { return &descriptor; }
uint64_t fixture_module_constructed(void) { return constructed; }
uint64_t fixture_module_destroyed(void) { return module_destroyed; }
uint64_t fixture_late_closed(void) { return late_closed; }
void fixture_module_mode(uint32_t value) { mode = value; }
void fixture_close_ready(uint32_t value) { close_ready = value != 0; }

static const rv_module_import counter_import = {
    .name = {(const uint8_t *)"counter", 7},
    .export_path = {(const uint8_t *)"device", 6},
    .declaration = {declaration, sizeof(declaration)},
    .access = 5, .create_access = 1, .retain_loans = 1
};
static const rv_module_dependency dependency = {
    .module = {0xb1}, .revision = 7, .scope = 1, .optional = 1,
    .imports = &counter_import, .import_count = 1
};
void fixture_module_declaration(rv_module *output) {
    *output = descriptor;
    output->dependencies = &dependency;
    output->dependency_count = 1;
}
