#include "reovim.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>

typedef struct { uint8_t first, second; } State;
typedef struct Context Context;
typedef struct Read {
    Context *owner;
    const State *state;
    struct Read *next;
    size_t size;
    uint8_t input[2], result[18], yielded;
} Read;
struct Context { size_t states; Read *reads; uint8_t result[18]; };
static void *create(const void *declaration,
                       const struct rv_connection *kernel) {
    (void)declaration;
    (void)kernel;
    return calloc(1, sizeof(Context)); }
static void destroy(void *pointer) {
    Context *context = pointer;
    assert(context->states == 0); /* State destructors must precede unload. */
    assert(context->reads == NULL);
    free(context);
}
static void drop_state(void *pointer, void *state) {
    Context *context = pointer;
    assert(context->states > 0);
    for (Read *read = context->reads; read; read = read->next)
        assert(read->state != state); /* Pending.release precedes state release. */
    --context->states;
    free(state);
}
static void release(void *context, struct rv_reply *reply) {
    if (reply->state) drop_state(context, reply->state);
}
static struct rv_reply allocate(Context *context, State value) {
    State *state = malloc(sizeof(State));
    if (!state) return (struct rv_reply){ .error = 5 };
    *state = value;
    ++context->states;
    return (struct rv_reply){ .state = state, .state_bytes = sizeof(State) };
}
static struct rv_reply initialize(void *context, const struct rv_request *request) {
    if (request->data.len) return (struct rv_reply){ .error = 3 };
    return allocate(context, (State){ 1, 0 });
}
static struct rv_reply prepare(void *context, const void *old,
                           const struct rv_request *request) {
    if (request->data.len != 8) return (struct rv_reply){ .error = 3 };
    uint64_t key = 0;
    for (unsigned i = 0; i < 8; ++i)
        key |= (uint64_t)request->data.data[i] << (8 * i);
    State next = *(const State *)old;
    if (key == 17) next.first = !next.first;
    else if (key == 9001) next.second = !next.second;
    else return (struct rv_reply){ .error = 3 };
    return allocate(context, next);
}
static struct rv_reply observe(void *pointer, const void *value,
                               const struct rv_request *request) {
    Context *context = pointer;
    const State *state = value;
    if (request->data.len) {
        if (request->data.len != 1 || request->data.data[0] > 1)
            return (struct rv_reply){ .error = 3 };
        context->result[0] = request->data.data[0] ? state->second : state->first;
        return (struct rv_reply){ .data = { context->result, 1 } };
    }
    memset(context->result, 0, sizeof(context->result));
    context->result[0] = 17;
    context->result[8] = state->first;
    context->result[9] = 9001 & 255;
    context->result[10] = 9001 >> 8;
    context->result[17] = state->second;
    return (struct rv_reply){ .data = { context->result, sizeof(context->result) } };
}
#define CONTRACT(name) { { (const uint8_t *)name, sizeof(name) - 1 }, 1 }
static struct rv_outcome poll_read(void *pointer, struct rv_wake wake) {
    Read *read = pointer;
    if (!read) return (struct rv_outcome){ .status = 3, .error = { .code = 5 } };
    if (!read->yielded) {
        read->yielded = 1;
        wake.wake(wake.context);
        return (struct rv_outcome){ .status = 0 };
    }
    struct rv_request input = { .method = 1,
        .contract = CONTRACT("native.keys"), .data = { read->input, read->size } };
    struct rv_reply value = observe(read->owner, read->state, &input);
    if (value.error) return (struct rv_outcome){ .status = 3,
        .error = { .code = value.error } };
    memcpy(read->result, value.data.data, value.data.len);
    return (struct rv_outcome){ .status = 1, .contract = CONTRACT("native.keys"),
        .data = { read->result, value.data.len } };
}
static void release_read(void *pointer) {
    Read *read = pointer;
    if (!read) return;
    Read **slot = &read->owner->reads;
    while (*slot != read) slot = &(*slot)->next;
    *slot = read->next;
    free(read);
}
static struct rv_pending inspect(void *pointer, const void *state,
                                 const struct rv_request *input) {
    Context *context = pointer;
    Read *read = calloc(1, sizeof(Read));
    if (read) {
        read->owner = context;
        read->state = state;
        read->size = input->data.len > 1 ? 2 : input->data.len;
        if (read->size) read->input[0] = input->data.data[0];
        read->next = context->reads;
        context->reads = read;
    }
    return (struct rv_pending){ read, poll_read, release_read };
}
static uint64_t unload(void *context) { (void)context; return 0; }
const struct rv_module *rv_module_entry(void) {
    static const struct rv_module module = {
        .version = RV_ABI_VERSION, .size = sizeof(struct rv_module), .kind = 2,
        .create_contract = CONTRACT("native.keys.create"),
        .read_contract = CONTRACT("native.keys"),
        .edit_contract = CONTRACT("native.keys.toggle"),
        .create = create, .destroy = destroy, .unload = unload,
        .initialize = initialize, .prepare = prepare, .observe = observe,
        .inspect = inspect,
        .release = release, .drop_state = drop_state,
    };
    return &module;
}
