#include "reovim.h"
#include <stdlib.h>
#include <stdatomic.h>

/* Independent encoding: Exec add(U32)->U64, progress U64; ioctl read()->U64.
 * The host has no API package dependency or knowledge of the counter state. */
static const uint8_t declaration[] = {
    0x71,0x71,0x71,0x71,0x71,0x71,0x71,0x71,
    0x71,0x71,0x71,0x71,0x71,0x71,0x71,0x71,
    2,0,0,0,
    1, 0,0,0,0, 0, 4,0,0,0, 2, 3,0,'a','d','d', 3,4,4,
    2, 1,0,0,0, 0, 1,0,0,0, 1, 4,0,'r','e','a','d', 0,4,0
};

typedef struct {
    uint64_t value, active;
    uint64_t *shared;
    void *owner;
    void (*release_owner)(void *);
    rv_interface interface;
} Counter;
typedef struct {
    Counter *counter;
    uint8_t value[8];
    uint32_t activity;
    int unknown, taken, outstanding;
} Request;
static uint64_t destroyed;
static uint64_t released;
static uint64_t finished;

static uint32_t poll(void *context, rv_wake wake) {
    (void)context;
    wake.retain(wake.context);
    wake.wake(wake.context);
    wake.release(wake.context);
    return RV_POLL_READY;
}
static void release_result(void *context) {
    Request *request = context;
    request->outstanding = 0;
    released++;
}
static uint32_t take(void *context, rv_outcome *output) {
    Request *request = context;
    if (request->taken) return RV_CLOSED;
    rv_error error = {0};
    if (request->unknown) {
        error.code = 90001; /* Unrecognized cause and optional metadata. */
        error.detail.epoch[0] = 0x51;
        error.detail.serial = 17;
    }
    *output = (rv_outcome){
        .status = request->unknown ? RV_OUTCOME_UNKNOWN : RV_OUTCOME_OK,
        .activity = request->activity,
        .error = error,
        .value = { .data = {request->value, sizeof(request->value)} },
        .owner = request, .release = release_result
    };
    request->taken = request->outstanding = 1;
    return RV_OK;
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
    return RV_OK; /* Refine evidence; do not change the counter again. */
}
static uint32_t finish(void *context) {
    Request *request = context;
    if (request->unknown || !request->taken || request->outstanding)
        return RV_BUSY;
    request->counter->active--;
    free(request);
    finished++;
    return RV_OK;
}
static const rv_pending_ops pending_ops = {
    sizeof(rv_pending_ops), poll, take, cancel, reconcile, finish
};
static uint32_t begin(Counter *counter, uint32_t add, int unknown,
                      rv_pending *output, rv_error *error) {
    Request *request = calloc(1, sizeof(*request));
    if (!request) {
        *error = (rv_error){ .code = RV_PRESSURE };
        return RV_PRESSURE;
    }
    if (counter->shared) counter->value = *counter->shared;
    counter->value += add;
    if (counter->shared) *counter->shared = counter->value;
    counter->active++;
    request->counter = counter;
    request->unknown = unknown;
    request->activity = unknown ? RV_WRITE : RV_READ;
    for (unsigned int i = 0; i < 8; i++)
        request->value[i] = (uint8_t)(counter->value >> (i * 8));
    *output = (rv_pending){ request, &pending_ops };
    return RV_OK;
}
static uint32_t exec(void *context, const rv_args *input,
                     rv_pending *output, rv_error *error) {
    if (input->data.len != 4 || input->refs.len != 0) {
        *error = (rv_error){ .code = RV_INVALID };
        return RV_INVALID;
    }
    uint32_t add = 0;
    for (unsigned int i = 0; i < 4; i++)
        add |= (uint32_t)input->data.ptr[i] << (i * 8);
    return begin(context, add, 1, output, error);
}
static uint32_t ioctl(void *context, uint32_t command, const rv_args *input,
                      rv_pending *output, rv_error *error) {
    if (command != 1 || input->data.len != 0 || input->refs.len != 0) {
        *error = (rv_error){ .code = RV_INVALID };
        return RV_INVALID;
    }
    return begin(context, 0, 0, output, error);
}
static const rv_exec_ops exec_ops = { sizeof(rv_exec_ops), exec };
static const rv_control_ops control_ops = { sizeof(rv_control_ops), ioctl };
static const rv_interface interfaces[] = {{
    .bytes = sizeof(rv_interface),
    .declaration = {declaration, sizeof(declaration)},
    .exec = &exec_ops, .control = &control_ops
}};
static uint32_t stop(void *context) {
    return ((Counter *)context)->active ? RV_BUSY : RV_OK;
}
static void destroy(void *context) {
    Counter *counter = context;
    if (counter->release_owner) counter->release_owner(counter->owner);
    free(context);
    destroyed++;
}

/* Test loader entry: returns one owned provider using only the public header. */
uint32_t fixture_create(rv_provider *output) {
    Counter *counter = calloc(1, sizeof(*counter));
    if (!counter) return RV_PRESSURE;
    counter->value = 41;
    counter->interface = interfaces[0];
    counter->interface.context = counter;
    *output = (rv_provider){
        .bytes = sizeof(rv_provider), .context = counter,
        .interfaces = &counter->interface, .interface_count = 1,
        .stop = stop, .destroy = destroy
    };
    return RV_OK;
}
/* Direct test observations of actual ownership events, not serialized sizes. */
uint64_t fixture_value(const rv_provider *provider) {
    return ((Counter *)provider->context)->value;
}
uint64_t fixture_destroyed(void) { return destroyed; }
uint64_t fixture_released(void) { return released; }
uint64_t fixture_finished(void) { return finished; }

/* Independent caller constructs the new ABI input on its own C stack. */
uint32_t fixture_invoke(const rv_caller *caller, uint64_t request,
    rv_access subject, uint32_t command, rv_pending *output, rv_error *error) {
    uint8_t input[] = {5, 0, 0, 0};
    rv_call call = {
        .request = request, .operation = 10, .subject = subject,
        .command = command, .input = {{input, sizeof(input)}, {NULL, 0}}
    };
    return caller->ops->submit(caller->context, &call, output, error);
}

static _Atomic uint64_t wake_refs, wakes;
static void signal_retain(void *context) {
    (void)context;
    atomic_fetch_add(&wake_refs, 1);
}
static void signal_release(void *context) {
    (void)context;
    atomic_fetch_sub(&wake_refs, 1);
}
static void signal_wake(void *context) {
    (void)context;
    atomic_fetch_add(&wakes, 1);
}
rv_wake fixture_signal(void) {
    return (rv_wake){NULL, signal_wake, signal_retain, signal_release};
}
uint64_t fixture_wake_refs(void) { return atomic_load(&wake_refs); }
uint64_t fixture_wakes(void) { return atomic_load(&wakes); }

typedef struct {
    Counter first, second;
    uint8_t second_declaration[sizeof(declaration)];
    rv_interface interfaces[2];
} Pair;
static uint32_t pair_stop(void *context) {
    Pair *pair = context;
    return pair->first.active || pair->second.active ? RV_BUSY : RV_OK;
}
uint32_t fixture_pair(rv_provider *output) {
    Pair *pair = calloc(1, sizeof(*pair));
    if (!pair) return RV_PRESSURE;
    pair->first.value = 41;
    pair->second.value = 83;
    for (unsigned i = 0; i < sizeof(declaration); i++)
        pair->second_declaration[i] = i < 16 ? 0x72 : declaration[i];
    pair->interfaces[0] = interfaces[0];
    pair->interfaces[0].context = &pair->first;
    pair->interfaces[1] = interfaces[0];
    pair->interfaces[1].context = &pair->second;
    pair->interfaces[1].declaration.ptr = pair->second_declaration;
    *output = (rv_provider){
        .bytes = sizeof(rv_provider), .context = pair,
        .interfaces = pair->interfaces, .interface_count = 2,
        .stop = pair_stop, .destroy = destroy
    };
    return RV_OK;
}
