#include "reovim.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>

typedef struct { uint8_t first, second; } State;
typedef struct Context Context;
typedef struct Preparing {
    Context *owner;
    const State *old;
    struct Preparing *next;
    struct rv_reply reply;
    size_t size;
    uint8_t input[8], yielded, ready, taken;
} Preparing;
typedef struct Read {
    Context *owner;
    const State *state;
    struct Read *next;
    size_t size;
    uint8_t input[2], result[18], yielded;
} Read;
struct Context {
    size_t states;
    Read *reads;
    Preparing *preparing;
    uint8_t result[18];
};
static void *create(const void *declaration,
                       const struct rv_connection *kernel) {
    (void)declaration;
    (void)kernel;
    return calloc(1, sizeof(Context)); }
static void destroy(void *pointer) {
    Context *context = pointer;
    assert(context->states == 0); /* State destructors must precede unload. */
    assert(context->reads == NULL);
    assert(context->preparing == NULL);
    free(context);
}
static void drop_state(void *pointer, void *state) {
    Context *context = pointer;
    assert(context->states > 0);
    for (Preparing *p = context->preparing; p; p = p->next)
        assert(p->old != state);
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
static struct rv_state_poll poll_state(void *pointer, struct rv_wake wake) {
    Preparing *p = pointer;
    if (!p) return (struct rv_state_poll){ .status = 2, .error = 5 };
    assert(!p->taken);
    if (!p->yielded) {
        p->yielded = 1;
        wake.wake(wake.context);
        return (struct rv_state_poll){ .status = 0 };
    }
    if (!p->ready) {
        struct rv_request input = { .data = { p->input, p->size } };
        p->reply = p->old ? prepare(p->owner, p->old, &input)
                         : initialize(p->owner, &input);
        p->ready = 1;
    }
    return (struct rv_state_poll){ .status = p->reply.error ? 2 : 1,
                                   .error = p->reply.error };
}
static struct rv_reply take_state(void *pointer) {
    Preparing *p = pointer;
    assert(p && p->ready && !p->taken);
    p->taken = 1;
    struct rv_reply reply = p->reply;
    p->reply.state = NULL;
    return reply;
}
static void release_state(void *pointer) {
    Preparing *p = pointer;
    if (!p) return;
    Preparing **slot = &p->owner->preparing;
    while (*slot != p) slot = &(*slot)->next;
    *slot = p->next;
    release(p->owner, &p->reply);
    free(p);
}
static struct rv_state_pending prepare_async(void *pointer, const void *old,
                                             const struct rv_request *input) {
    Context *context = pointer;
    Preparing *p = calloc(1, sizeof(Preparing));
    if (p) {
        p->owner = context;
        p->old = old;
        p->size = input->data.len > 8 ? 9 : input->data.len;
        if (p->size <= 8 && p->size)
            memcpy(p->input, input->data.data, p->size);
        p->next = context->preparing;
        context->preparing = p;
    }
    return (struct rv_state_pending){ p, poll_state, take_state, release_state };
}
static struct rv_state_pending initialize_async(void *context,
                                                const struct rv_request *input) {
    return prepare_async(context, NULL, input);
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
        .initialize_async = initialize_async, .prepare_async = prepare_async,
        .observe = observe,
        .inspect = inspect,
        .release = release, .drop_state = drop_state,
    };
    return &module;
}
