#include "operations.c"
#include <string.h>

/* This artifact consumes another module solely through its granted Caller. */
typedef struct {
    /* Shared counter callbacks use the initial member as their context. */
    Counter copied;
    rv_caller caller;
    rv_registrar exports;
    rv_pending pending;
    uint64_t request;
    rv_access access;
    uint32_t phase, step;
    uint64_t value;
    int published, finishing, copied_destroyed;
    uint32_t completion;
} Reader;

static void put32(uint8_t *bytes, uint32_t value) {
    for (unsigned i = 0; i < 4; i++) bytes[i] = (uint8_t)(value >> (8 * i));
}
static uint32_t submit_next(Reader *reader, rv_error *error) {
    uint8_t input[sizeof(declaration) + 32] = {0};
    rv_call call = {.subject = reader->access};
    size_t size = 0;
    if (reader->step == 0) {
        const char *path = "/imports/input";
        put32(input + 4, (uint32_t)strlen(path));
        memcpy(input + 8, path, strlen(path));
        size = 8 + strlen(path);
        call.operation = 1;
    } else if (reader->step == 1) {
        put32(input, RV_READ);
        size = 4;
        call.operation = 21;
    } else if (reader->step == 2) {
        put32(input, sizeof(declaration));
        memcpy(input + 4, declaration, sizeof(declaration));
        size = 4 + sizeof(declaration);
        call.operation = 3;
    } else {
        call.operation = 11;
        call.command = 1;
    }
    uint32_t status = reader->caller.ops->reserve(reader->caller.context,
                                                 &reader->request);
    if (status != RV_OK) return status;
    call.request = reader->request;
    call.input.data = (rv_bytes){input, size};
    status = reader->caller.ops->submit(reader->caller.context, &call,
                                        &reader->pending, error);
    if (status != RV_OK) {
        reader->caller.ops->release(reader->caller.context, reader->request);
        reader->request = 0;
    }
    return status;
}
static uint32_t finish_view(Reader *reader) {
    uint32_t status = reader->pending.ops->finish(reader->pending.context);
    if (status == RV_BUSY) return RV_NOT_READY;
    if (status != RV_OK) return status;
    reader->pending = (rv_pending){0};
    if (reader->caller.ops->release(reader->caller.context,
                                    reader->request) != RV_OK) abort();
    reader->request = 0;
    reader->finishing = 0;
    return reader->completion;
}
static uint32_t receive(Reader *reader, rv_wake wake) {
    if (reader->finishing) return finish_view(reader);
    uint32_t polled = reader->pending.ops->poll(reader->pending.context, wake);
    if (polled == RV_POLL_PENDING) return RV_NOT_READY;
    if (polled != RV_POLL_READY) return polled;
    rv_outcome outcome = {0};
    uint32_t status = reader->pending.ops->take(reader->pending.context,
                                               &outcome);
    if (status != RV_OK) return status;
    if (outcome.status != RV_OUTCOME_OK) {
        status = outcome.status == RV_OUTCOME_FAILED
            ? outcome.error.code : RV_INCOMPLETE;
    } else if (reader->step == 0 || reader->step == 2) {
        if (outcome.value.refs.len != 1) abort();
        reader->access = outcome.value.refs.ptr[0];
    } else if (reader->step == 3) {
        if (outcome.value.data.len != 8) abort();
        reader->value = 0;
        for (unsigned i = 0; i < 8; i++)
            reader->value |= (uint64_t)outcome.value.data.ptr[i] << (8 * i);
    }
    if (status == RV_OK && reader->caller.ops->adopt(reader->caller.context,
                                                   reader->request)) abort();
    if (outcome.release) outcome.release(outcome.owner);
    reader->completion = status;
    reader->finishing = 1;
    return finish_view(reader);
}
static void copied_destroy(void *context) {
    Reader *reader = context;
    if (reader->copied_destroyed || reader->phase == RV_MODULE_CLOSED) abort();
    reader->copied_destroyed = 1;
}
static uint32_t publish(Reader *reader) {
    reader->copied.value = reader->value;
    reader->copied.interface = interfaces[0];
    reader->copied.interface.context = reader;
    rv_provider provider = {
        .bytes = sizeof(provider), .context = reader,
        .interfaces = &reader->copied.interface, .interface_count = 1,
        .stop = stop, .destroy = copied_destroy
    };
    uint32_t status = reader->exports.stage(reader->exports.context,
        (rv_bytes){(const uint8_t *)"copied", 6}, &provider);
    if (status != RV_OK) {
        provider.destroy(provider.context);
        return status;
    }
    reader->published = 1;
    reader->phase = RV_READY;
    return RV_OK;
}
static uint32_t reader_status(void *context, rv_module_status *status) {
    *status = (rv_module_status){.phase = ((Reader *)context)->phase};
    return RV_OK;
}
static uint32_t reader_advance(void *context, rv_wake wake,
                                uint32_t *progress, rv_error *error) {
    Reader *reader = context;
    *progress = RV_IDLE;
    if (reader->phase >= RV_READY) return RV_OK;
    reader->phase = RV_STARTING;
    uint32_t status;
    if (reader->pending.ops) {
        status = receive(reader, wake);
        if (status == RV_NOT_READY) return RV_OK;
        if (status == RV_OK) reader->step++;
    } else {
        status = reader->step == 4 ? publish(reader)
                                  : submit_next(reader, error);
    }
    if (status != RV_OK) { error->code = status; return status; }
    *progress = RV_MORE;
    return RV_OK;
}
static uint32_t reader_close(void *context, rv_error *error) {
    Reader *reader = context;
    reader->phase = RV_CLOSING;
    if (reader->pending.ops) {
        reader->caller.ops->cancel(reader->caller.context, reader->request);
        uint32_t status = reader->pending.ops->finish(reader->pending.context);
        if (status != RV_OK) { error->code = status; return status; }
        reader->pending = (rv_pending){0};
        if (reader->caller.ops->release(reader->caller.context,
                                        reader->request) != RV_OK) abort();
        reader->request = 0;
    }
    /* Runtime must destroy published providers before module close. */
    if (reader->published && !reader->copied_destroyed) abort();
    reader->phase = RV_MODULE_CLOSED;
    return RV_OK;
}
static void reader_destroy(void *context) {
    Reader *reader = context;
    if (reader->phase != RV_MODULE_CLOSED || reader->request
        || reader->pending.ops) abort();
    free(reader);
}
static const rv_module_driver reader_driver = {
    sizeof(rv_module_driver), reader_status, reader_advance, reader_close
};
static uint32_t reader_construct(const rv_module_context *context,
    rv_module_instance *output, rv_error *error) {
    if (!context || context->bytes != sizeof(*context)) {
        error->code = RV_INVALID;
        return RV_INVALID;
    }
    Reader *reader = calloc(1, sizeof(*reader));
    if (!reader) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    reader->caller = context->imports;
    reader->exports = context->exports;
    *output = (rv_module_instance){sizeof(*output), reader, &reader_driver,
                                   reader_destroy};
    return RV_OK;
}
static const rv_module_import imported = {
    .name = {(const uint8_t *)"input", 5},
    .export_path = {(const uint8_t *)"first", 5},
    .declaration = {declaration, sizeof(declaration)}, .access = RV_READ
};
static const rv_module_dependency dependency = {
    .module = {0xa3}, .revision = 1, .scope = 1,
    .imports = &imported, .import_count = 1
};
static const rv_bytes required[] = {{(const uint8_t *)"copied", 6}};
static const rv_module descriptor = {
    .bytes = sizeof(rv_module), .version = RV_ABI_VERSION, .id = {0xa4},
    .name = {(const uint8_t *)"reader", 6}, .revision = 1, .scopes = 1,
    .dependencies = &dependency, .dependency_count = 1,
    .required = required, .required_count = 1, .export_limit = 1,
    .construct = reader_construct
};
const rv_module *rv_module_entry(void) { return &descriptor; }
