#include "reovim.h"
#include <assert.h>
#include <stdlib.h>

#define CONTRACT(s) { { (const uint8_t *)s, sizeof(s) - 1 }, 1 }
typedef struct { struct rv_connection kernel; size_t states; } Model;
typedef struct {
    Model *owner;
    uint8_t value;
    size_t count;
    uint64_t ids[2];
    struct rv_connection captures[2];
} State;
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
    assert(model->states == 0);
    model->kernel.table->release(model->kernel.context);
    free(model);
}
static void drop_state(void *pointer, void *value) {
    Model *model = pointer;
    State *state = value;
    assert(state->owner == model && model->states);
    for (size_t i = 0; i < state->count; ++i)
        state->captures[i].table->release(state->captures[i].context);
    --model->states;
    free(state);
}
static struct rv_reply initialize(void *pointer, const struct rv_request *input) {
    Model *model = pointer;
    if (input->data.len != 1 || input->resource_count > 2)
        return (struct rv_reply){ .error = 3 };
    State *state = calloc(1, sizeof(State));
    if (!state) return (struct rv_reply){ .error = 5 };
    state->owner = model;
    state->value = input->data.data[0];
    ++model->states;
    for (size_t i = 0; i < input->resource_count; ++i) {
        struct rv_failure result = model->kernel.table->capture(
            model->kernel.context, input->resources[i], 1, &state->captures[i]);
        if (result.code) {
            drop_state(model, state);
            return (struct rv_reply){ .error = result.code };
        }
        state->ids[i] = input->resources[i];
        ++state->count;
    }
    return (struct rv_reply){ .state = state, .state_bytes = sizeof(State) };
}
static struct rv_reply prepare(void *model, const void *old,
                               const struct rv_request *input) {
    (void)old;
    return initialize(model, input);
}
static struct rv_reply observe(void *model, const void *pointer,
                               const struct rv_request *input) {
    (void)model; (void)input;
    const State *state = pointer;
    return (struct rv_reply){ .data = { &state->value, 1 },
        .resources = state->ids, .resource_count = state->count };
}
static void release(void *model, struct rv_reply *reply) {
    if (reply->state) drop_state(model, reply->state);
}
static uint64_t unload(void *model) { (void)model; return 0; }
const struct rv_module *rv_module_entry(void) {
    static const struct rv_module table = {
        .version = RV_ABI_VERSION, .size = sizeof(struct rv_module), .kind = 2,
        .create_contract = CONTRACT("native.references.initial"),
        .read_contract = CONTRACT("native.references"),
        .edit_contract = CONTRACT("native.references.update"),
        .create = create, .destroy = destroy, .initialize = initialize,
        .prepare = prepare, .observe = observe, .release = release,
        .drop_state = drop_state, .unload = unload, .references = 1,
    };
    return &table;
}
