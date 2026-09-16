#include "open.c"

typedef struct { Device device; int created; } Directory;
static uint64_t materializations, creations;
static uint32_t flags(void *context, rv_pending *output, rv_error *error) {
    Request *request = allocate(context, error);
    if (!request) return RV_PRESSURE;
    request->data[0] = 1;
    request->size = 4;
    *output = (rv_pending){request, &pending_ops};
    return RV_OK;
}
static uint32_t lookup(void *context, rv_bytes name,
    rv_pending *output, rv_error *error) {
    Directory *directory = context;
    int created = directory->created && name.len == 3 && !memcmp(name.ptr, "new", 3);
    if (!created && (name.len != 4 || memcmp(name.ptr, "item", 4))) {
        error->code = RV_UNAVAILABLE; return RV_UNAVAILABLE;
    }
    Request *request = allocate(context, error);
    if (!request) return RV_PRESSURE;
    request->data[0] = 1;
    request->data[4] = created ? 'n' : 'k';
    request->size = 5;
    *output = (rv_pending){request, &pending_ops};
    return RV_OK;
}
static uint32_t list_take(void *context, rv_outcome *output) {
    static const uint8_t page[] = {
        1,0,0,0, 4,0,0,0, 'i','t','e','m', 1,0,0,0,'k',
        0,0,0,0,0,0,0,0
    };
    static const uint8_t created[] = {
        2,0,0,0, 4,0,0,0, 'i','t','e','m', 1,0,0,0,'k',
        3,0,0,0,'n','e','w', 1,0,0,0,'n', 0,0,0,0,0,0,0,0
    };
    uint32_t result = take(context, output);
    if (!result) output->value.data = ((Request *)context)->data[0] ?
        (rv_bytes){created, sizeof(created)} : (rv_bytes){page, sizeof(page)};
    return result;
}
static const rv_pending_ops list_ops = {
    sizeof(rv_pending_ops), poll, list_take, cancel, reconcile, finish
};
static uint32_t list(void *context, uint64_t cursor, uint64_t maximum,
    rv_pending *output, rv_error *error) {
    size_t needed = ((Directory *)context)->created ? 37 : 25;
    if (cursor || maximum < needed) {
        error->code = RV_INVALID; return RV_INVALID;
    }
    Request *request = allocate(context, error);
    if (!request) return RV_PRESSURE;
    request->data[0] = ((Directory *)context)->created;
    request->activity = RV_READ;
    *output = (rv_pending){request, &list_ops};
    return RV_OK;
}
static uint32_t materialize(void *context, rv_bytes key,
    const rv_creation *creation, rv_pending *output, rv_error *error) {
    if (key.len != 1 || (key.ptr[0] != 'k' &&
        (key.ptr[0] != 'n' || !((Directory *)context)->created))) {
        error->code = RV_UNAVAILABLE; return RV_UNAVAILABLE;
    }
    Request *request = allocate(context, error);
    if (!request) return RV_PRESSURE;
    rv_provider provider;
    uint32_t result = fixture_open_offered(&provider);
    if (!result) {
        result = creation->resource(creation->context, &provider);
        if (result) { provider.stop(provider.context); provider.destroy(provider.context); }
    }
    if (result) {
        request->device->active--; free(request);
        error->code = result; return result;
    }
    materializations++;
    request->activity = RV_READ;
    *output = (rv_pending){request, &pending_ops};
    return RV_OK;
}
static uint32_t create_name(void *context, rv_bytes name,
    const rv_creation *creation, rv_pending *output, rv_error *error) {
    Directory *directory = context;
    if (directory->created) { error->code = RV_CONFLICT; return RV_CONFLICT; }
    if (name.len != 3 || memcmp(name.ptr, "new", 3) || !creation->keyed) {
        error->code = RV_INVALID; return RV_INVALID;
    }
    Request *request = allocate(context, error);
    if (!request) return RV_PRESSURE;
    rv_provider provider = {0};
    uint32_t result = fixture_open_offered(&provider);
    if (!result) result = creation->keyed(creation->context,
        (rv_bytes){(const uint8_t *)"n", 1}, &provider);
    if (result) {
        if (provider.context) { provider.stop(provider.context); provider.destroy(provider.context); }
        request->device->active--; free(request);
        error->code = result; return result;
    }
    directory->created = 1;
    creations++;
    request->activity = RV_WRITE;
    *output = (rv_pending){request, &pending_ops};
    return RV_OK;
}
static const rv_namespace_ops namespace_ops = {
    .bytes = sizeof(rv_namespace_ops), .flags = flags, .lookup = lookup,
    .list = list, .materialize = materialize, .create = create_name
};
uint32_t fixture_directory(rv_provider *output) {
    Directory *directory = calloc(1, sizeof(*directory));
    if (!directory) return RV_PRESSURE;
    *output = (rv_provider){ .bytes = sizeof(rv_provider),
        .context = directory, .namespace_ops = &namespace_ops,
        .stop = stop, .destroy = destroy };
    return RV_OK;
}
uint64_t fixture_materializations(void) { return materializations; }
uint64_t fixture_creations(void) { return creations; }
