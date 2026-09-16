#include "reovim.h"
#include <assert.h>
#include <stdlib.h>

/* Independent grammar: two Factory selectors return Ref(child, READ) + U64.
 * mode is fixture input, not a Kernel command or a domain string. */
static const uint8_t factory_declaration[] = {
    0x74,0x74,0x74,0x74,0x74,0x74,0x74,0x74,
    0x74,0x74,0x74,0x74,0x74,0x74,0x74,0x74, 3,0,0,0,
    1, 0,0,0,0, 2, 4,0,0,0, 0, 4,0,'m','a','k','e', 3,4,4,
    12, 0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75,
    0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75, 1,0,0,0,
    2, 7,0,0,0, 2, 1,0,0,0, 0, 4,0,'c','o','p','y', 3,4,4,
    12, 0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75,
    0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75, 1,0,0,0,
    2, 8,0,0,0, 2, 1,0,0,0, 0, 3,0,'s','e','t', 3,
    10, 2,0,0,0, 12,
    0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75,
    0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75, 1,0,0,0, 4,
    12, 0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75,
    0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75, 1,0,0,0
};
static const uint8_t child_declaration[] = {
    0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75,
    0x75,0x75,0x75,0x75,0x75,0x75,0x75,0x75, 1,0,0,0,
    2, 1,0,0,0, 0, 1,0,0,0, 0, 4,0,'r','e','a','d', 0,4,0
};

typedef struct { uint64_t active; rv_interface interface; } Factory;
typedef struct Child {
    uint64_t active, value; int rejected, stopped;
    rv_interface interface;
#ifdef RV_FACTORY_OPEN
    struct Child *parent;
#endif
} Child;
typedef struct {
    Factory *factory;
    Child *child;
    const rv_creation *creation;
    uint8_t value[12];
    uint64_t tokens[2];
    int auxiliary;
    uint32_t mode;
    int staged, taken, outstanding, unknown;
} Request;

static uint64_t created, child_destroyed, factory_destroyed;
static uint64_t released, finished, rejected_destroyed, late_rejected;
static int cleanup_ready = 1;
static uint64_t auxiliary_tokens[2];
static rv_provider child_provider(int rejected);
#ifdef RV_FACTORY_OPEN
static const rv_open_ops child_open_ops;
static uint64_t opened_destroyed;
#endif

static uint32_t child_stop(void *context) {
    Child *child = context;
    if (child->active || (!child->rejected && !cleanup_ready)) return RV_BUSY;
    child->stopped = 1;
    return RV_OK;
}
static void child_destroy(void *context) {
    Child *child = context;
    assert(child->stopped && !child->active);
#ifdef RV_FACTORY_OPEN
    if (child->parent) {
        child->parent->active--;
        opened_destroyed++;
        free(child);
        return;
    }
#endif
    if (child->rejected) rejected_destroyed++;
    else child_destroyed++;
    free(child);
}
static void dispose(rv_provider provider) {
    assert(provider.stop(provider.context) == RV_OK);
    provider.destroy(provider.context);
}
static void rejected(const rv_creation *creation, uint32_t expected) {
    rv_provider provider = child_provider(1);
    if (expected == RV_UNSUPPORTED) provider.interface_count = 0;
    uint64_t before = rejected_destroyed;
    assert(creation->resource(creation->context, &provider) == expected);
    assert(rejected_destroyed == before);
    assert(!((Child *)provider.context)->stopped);
    dispose(provider);
}
static void stage_auxiliary(Request *request) {
    const rv_creation *creation = request->creation;
    for (unsigned int i = 0; i < 2; i++) {
        rv_provider provider = child_provider(0);
        ((Child *)provider.context)->value = 0xabc000 + i;
        assert(creation->auxiliary(creation->context, &provider,
                                  &request->tokens[i]) == RV_OK);
        auxiliary_tokens[i] = request->tokens[i];
    }
    rv_provider rejected = child_provider(1);
    uint64_t untouched = UINT64_MAX;
    assert(creation->auxiliary(creation->context, &rejected, &untouched)
           == RV_PRESSURE);
    assert(untouched == UINT64_MAX);
    assert(!((Child *)rejected.context)->stopped);
    dispose(rejected);
}
static uint32_t poll(void *context, rv_wake wake) {
    Request *request = context;
    (void)wake;
    if (request->creation && !request->staged) {
        const rv_creation *creation = request->creation;
        rejected(creation, RV_UNSUPPORTED);
        if (request->mode != 2 && request->mode != 11) {
            rv_provider child = child_provider(0);
            assert(creation->resource(creation->context, &child) == RV_OK);
            rejected(creation, RV_CONFLICT);
        }
        if (request->auxiliary) stage_auxiliary(request);
        request->staged = 1;
    }
    return RV_POLL_READY;
}
static void release_result(void *context) {
    Request *request = context;
    assert(request->outstanding);
    request->outstanding = 0;
    released++;
}
static uint32_t take(void *context, rv_outcome *output) {
    Request *request = context;
    if (request->taken) return RV_CLOSED;
    uint32_t status = RV_OUTCOME_OK;
    rv_error error = {0};
    if (request->unknown) {
        status = RV_OUTCOME_UNKNOWN;
        error.code = RV_UNAVAILABLE;
    } else if (request->mode == 3 || request->mode == 8) {
        status = RV_OUTCOME_FAILED;
        error.code = RV_UNAVAILABLE;
    }
    *output = (rv_outcome){
        .status = status, .error = error,
        .value = {.data = {request->value,
                          status == RV_OUTCOME_FAILED ? 0 : 8}},
        .owner = request, .release = release_result
    };
    if (request->auxiliary && status == RV_OUTCOME_OK) {
        unsigned int count = request->mode == 6 ? 1 : 2;
        if (request->mode == 7) count = 0;
        for (unsigned int i = 0; i < 12; i++) request->value[i] = 0;
        request->value[0] = (uint8_t)count;
        request->value[8] = 1;
        output->value.data.len = 4 + count * 4;
        output->value.refs = (rv_refs){request->tokens, count};
        if (request->mode == 10) request->tokens[0] = UINT64_MAX;
    }
    /* Malformed metadata must leave the result owner retained by the host. */
    if (request->mode == 4) output->activity = 8;
    request->taken = request->outstanding = 1;
    return request->mode == 5 ? RV_PROVIDER : RV_OK;
}
static uint32_t cancel(void *context) {
    (void)context;
    return RV_CANCEL_TOO_LATE;
}
static uint32_t reconcile(void *context) {
    Request *request = context;
    if (!request->unknown || !request->taken) return RV_CLOSED;
    if (request->outstanding) return RV_BUSY;
    request->unknown = request->taken = 0;
    return RV_OK;
}
static uint32_t finish(void *context) {
    Request *request = context;
    if (request->unknown || !request->taken || request->outstanding)
        return RV_BUSY;
    if (request->mode == 8) assert(child_destroyed == 3);
    if (request->mode == 11) assert(child_destroyed == 2);
    if (request->creation) {
        /* The host has fenced staging, but the exported pointer still lives. */
        rejected(request->creation, RV_CLOSED);
        late_rejected++;
    }
    if (request->factory) request->factory->active--;
    if (request->child) request->child->active--;
    free(request);
    finished++;
    return RV_OK;
}
static const rv_pending_ops pending_ops = {
    sizeof(rv_pending_ops), poll, take, cancel, reconcile, finish
};
static void encode(Request *request, uint64_t value) {
    for (unsigned int i = 0; i < 8; i++)
        request->value[i] = (uint8_t)(value >> (i * 8));
}
static uint32_t read_child(void *context, uint32_t command,
                           const rv_args *input, rv_pending *output,
                           rv_error *error) {
    assert(command == 1 && input->data.len == 0 && input->refs.len == 0);
#ifdef RV_FACTORY_OPEN
    if (!((Child *)context)->parent) {
        error->code = RV_UNSUPPORTED;
        return RV_UNSUPPORTED;
    }
#endif
    Request *request = calloc(1, sizeof(*request));
    if (!request) {
        *error = (rv_error){.code = RV_PRESSURE};
        return RV_PRESSURE;
    }
    request->child = context;
    request->child->active++;
    encode(request, request->child->value);
    *output = (rv_pending){request, &pending_ops};
    return RV_OK;
}
static const rv_control_ops child_control = {
    sizeof(rv_control_ops), read_child
};
static const rv_interface child_interfaces[] = {{
    .bytes = sizeof(rv_interface),
    .declaration = {child_declaration, sizeof(child_declaration)},
    .control = &child_control
}};
static rv_provider child_provider(int rejected) {
    Child *child = calloc(1, sizeof(*child));
    assert(child);
    child->rejected = rejected;
    child->value = 0xfeed1234;
    child->interface = child_interfaces[0];
    child->interface.context = child;
    return (rv_provider){
        .bytes = sizeof(rv_provider), .context = child,
        .interfaces = &child->interface, .interface_count = 1,
#ifdef RV_FACTORY_OPEN
        .open = &child_open_ops,
#endif
        .stop = child_stop, .destroy = child_destroy
    };
}
static uint32_t create(void *context, uint32_t command,
                       const rv_creation *creation, const rv_args *input,
                       rv_pending *output, rv_error *error) {
    assert(command == 0 || command == 7 || command == 8);
    assert(input->data.len == 4 && input->refs.len == 0);
    Request *request = calloc(1, sizeof(*request));
    if (!request) {
        *error = (rv_error){.code = RV_PRESSURE};
        return RV_PRESSURE;
    }
    request->factory = context;
    request->factory->active++;
    request->creation = creation;
    request->mode = input->data.ptr[0];
    request->auxiliary = command == 8;
    request->unknown = request->mode == 1 || request->mode == 9;
    encode(request, 0xcafe5678);
    created++;
    *output = (rv_pending){request, &pending_ops};
    return RV_OK;
}
static const rv_factory_ops factory_ops = {sizeof(rv_factory_ops), create};
static const rv_interface factory_interfaces[] = {{
    .bytes = sizeof(rv_interface),
    .declaration = {factory_declaration, sizeof(factory_declaration)},
    .factory = &factory_ops
}};
static uint32_t stop(void *context) {
    return ((Factory *)context)->active ? RV_BUSY : RV_OK;
}
static void destroy(void *context) {
    assert(!((Factory *)context)->active);
    factory_destroyed++;
    free(context);
}
uint32_t fixture_create(rv_provider *output) {
    Factory *factory = calloc(1, sizeof(*factory));
    if (!factory) return RV_PRESSURE;
    factory->interface = factory_interfaces[0];
    factory->interface.context = factory;
    *output = (rv_provider){
        .bytes = sizeof(rv_provider), .context = factory,
        .interfaces = &factory->interface, .interface_count = 1,
        .stop = stop, .destroy = destroy
    };
    return RV_OK;
}
void fixture_cleanup_ready(uint32_t ready) { cleanup_ready = ready != 0; }
uint64_t fixture_created(void) { return created; }
uint64_t fixture_child_destroyed(void) { return child_destroyed; }
uint64_t fixture_factory_destroyed(void) { return factory_destroyed; }
uint64_t fixture_released(void) { return released; }
uint64_t fixture_finished(void) { return finished; }
uint64_t fixture_late_rejected(void) { return late_rejected; }
uint64_t fixture_rejected_destroyed(void) { return rejected_destroyed; }

uint64_t fixture_auxiliary_token(uint32_t index) {
    assert(index < 2);
    return auxiliary_tokens[index];
}
