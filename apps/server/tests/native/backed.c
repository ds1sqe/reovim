#include "reovim.h"
#include <assert.h>
#include <stdlib.h>

#define CONTRACT(s) { { (const uint8_t *)s, sizeof(s) - 1 }, 1 }
typedef struct { struct rv_connection kernel; size_t states, pending; } Model;
typedef struct {
    Model *model;
    struct rv_connection backing;
    uint64_t resource;
    uint8_t value;
} State;
typedef struct {
    Model *model;
    State *state;
    struct rv_pending read;
    uint64_t error;
    int ready;
} Preparing;

static void *create(const void *declaration, const struct rv_connection *kernel) {
    (void)declaration;
    if (!kernel) return NULL;
    Model *model = calloc(1, sizeof(Model));
    if (!model) return NULL;
    model->kernel = *kernel;
    kernel->table->retain(kernel->context);
    return model;
}
static void destroy(void *pointer) {
    Model *model = pointer;
    assert(!model->states && !model->pending);
    model->kernel.table->release(model->kernel.context);
    free(model);
}
static void drop_state(void *pointer, void *value) {
    Model *model = pointer;
    State *state = value;
    assert(model == state->model && model->states);
    state->backing.table->release(state->backing.context);
    --model->states;
    free(state);
}
static void release(void *model, struct rv_reply *reply) {
    if (reply->state) drop_state(model, reply->state);
}
static struct rv_request read_request(uint64_t id) {
    return (struct rv_request){ .method = 1, .subject = id,
        .contract = CONTRACT("resource.bytes") };
}
static struct rv_state_poll poll(void *pointer, struct rv_wake wake) {
    Preparing *p = pointer;
    if (!p) return (struct rv_state_poll){ .status = 2, .error = 5 };
    if (!p->ready && !p->error) {
        struct rv_outcome read = p->read.poll(p->read.context, wake);
        if (!read.status) return (struct rv_state_poll){ 0 };
        if (read.status != 1) p->error = read.error.code ? read.error.code : 3;
        else if (read.data.len != 1) p->error = 3;
        else { p->state->value = read.data.data[0]; p->ready = 1; }
    }
    return (struct rv_state_poll){ .status = p->error ? 2 : 1,
                                  .error = p->error };
}
static struct rv_reply take(void *pointer) {
    Preparing *p = pointer;
    assert(p && p->ready && p->state);
    State *state = p->state;
    p->state = NULL;
    return (struct rv_reply){ .state = state, .state_bytes = sizeof(State) };
}
static void release_pending(void *pointer) {
    Preparing *p = pointer;
    if (!p) return;
    if (p->read.release) p->read.release(p->read.context);
    if (p->state) drop_state(p->model, p->state);
    assert(p->model->pending);
    --p->model->pending;
    free(p);
}
static struct rv_state_pending initialize(void *pointer,
                                          const struct rv_request *input) {
    Model *model = pointer;
    Preparing *p = calloc(1, sizeof(Preparing));
    struct rv_state_pending result = { p, poll, take, release_pending };
    if (!p) return result;
    p->model = model;
    ++model->pending;
    p->error = 3;
    if (input->resource_count != 1) return result;
    State *state = calloc(1, sizeof(State));
    if (!state) { p->error = 5; return result; }
    struct rv_failure captured = model->kernel.table->capture(
        model->kernel.context, input->resources[0], 1, &state->backing);
    if (captured.code) { p->error = captured.code; free(state); return result; }
    state->model = model;
    state->resource = input->resources[0];
    ++model->states;
    p->state = state;
    p->error = 0;
    struct rv_request read = read_request(state->resource);
    p->read = state->backing.table->request(state->backing.context, &read);
    return result;
}
static struct rv_reply prepare(void *model, const void *old,
                               const struct rv_request *input) {
    (void)model; (void)old; (void)input;
    return (struct rv_reply){ .error = 2 };
}
static struct rv_reply observe(void *model, const void *pointer,
                               const struct rv_request *input) {
    (void)model; (void)input;
    const State *state = pointer;
    return (struct rv_reply){ .data = { &state->value, 1 } };
}
typedef struct { struct rv_pending read; const State *state; } Inspect;
static struct rv_outcome poll_inspect(void *pointer, struct rv_wake wake) {
    Inspect *p = pointer;
    if (!p) return (struct rv_outcome){ .status = 3, .error = { .code = 5 } };
    struct rv_outcome value = p->read.poll(p->read.context, wake);
    if (value.status == 1) {
        if (value.data.len != 1 || value.data.data[0] != p->state->value)
            return (struct rv_outcome){ .status = 3, .error = { .code = 7 } };
        value.contract = (struct rv_contract)CONTRACT("backed.value");
    }
    return value;
}
static void release_inspect(void *pointer) {
    Inspect *p = pointer;
    if (!p) return;
    p->read.release(p->read.context);
    free(p);
}
static struct rv_pending inspect(void *model, const void *pointer,
                                 const struct rv_request *input) {
    (void)model; (void)input;
    const State *state = pointer;
    Inspect *p = calloc(1, sizeof(Inspect));
    if (p) {
        p->state = state;
        struct rv_request read = read_request(state->resource);
        p->read = state->backing.table->request(state->backing.context, &read);
    }
    return (struct rv_pending){ p, poll_inspect, release_inspect };
}
static uint64_t unload(void *model) { (void)model; return 0; }
const struct rv_module *rv_module_entry(void) {
    static const uint32_t input[] = { 1 };
    static const struct rv_module table = {
        .version = RV_ABI_VERSION, .size = sizeof(struct rv_module), .kind = 2,
        .create_contract = CONTRACT("backed.initial"),
        .initial_input = input, .initial_input_count = 1,
        .read_contract = CONTRACT("backed.value"),
        .edit_contract = CONTRACT("backed.edit"),
        .create = create, .destroy = destroy, .initialize_async = initialize,
        .prepare = prepare, .observe = observe, .inspect = inspect,
        .release = release, .drop_state = drop_state, .unload = unload,
    };
    return &table;
}
