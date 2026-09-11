#include "reovim.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>

typedef struct { uint8_t first, second; } State;
typedef struct { size_t states; uint8_t result[18]; } Context;
static void *create(const ReovimConnection *kernel) {
    (void)kernel;
    return calloc(1, sizeof(Context)); }
static void destroy(void *pointer) {
    Context *context = pointer;
    assert(context->states == 0); /* State destructors must precede unload. */
    free(context);
}
static void drop_state(void *pointer, void *state) {
    Context *context = pointer;
    assert(context->states > 0);
    --context->states;
    free(state);
}
static void release(void *context, ReovimReply *reply) {
    if (reply->state) drop_state(context, reply->state);
}
static ReovimReply allocate(Context *context, State value) {
    State *state = malloc(sizeof(State));
    if (!state) return (ReovimReply){ .error = 5 };
    *state = value;
    ++context->states;
    return (ReovimReply){ .state = state, .state_bytes = sizeof(State) };
}
static ReovimReply initialize(void *context, const ReovimRequest *request) {
    if (request->data.len) return (ReovimReply){ .error = 3 };
    return allocate(context, (State){ 1, 0 });
}
static ReovimReply prepare(void *context, const void *old,
                           const ReovimRequest *request) {
    if (request->data.len != 8) return (ReovimReply){ .error = 3 };
    uint64_t key = 0;
    for (unsigned i = 0; i < 8; ++i)
        key |= (uint64_t)request->data.data[i] << (8 * i);
    State next = *(const State *)old;
    if (key == 17) next.first = !next.first;
    else if (key == 9001) next.second = !next.second;
    else return (ReovimReply){ .error = 3 };
    return allocate(context, next);
}
static ReovimReply observe(void *pointer, const void *value) {
    Context *context = pointer;
    const State *state = value;
    memset(context->result, 0, sizeof(context->result));
    context->result[0] = 17;
    context->result[8] = state->first;
    context->result[9] = 9001 & 255;
    context->result[10] = 9001 >> 8;
    context->result[17] = state->second;
    return (ReovimReply){ .data = { context->result, sizeof(context->result) } };
}
#define CONTRACT(name) { { (const uint8_t *)name, sizeof(name) - 1 }, 1 }
static uint64_t unload(void *context) { (void)context; return 0; }
const ReovimModule *reovim_module_v2(void) {
    static const ReovimModule module = {
        .version = 2, .size = sizeof(ReovimModule), .kind = 2,
        .create_contract = CONTRACT("native.keys.create"),
        .read_contract = CONTRACT("native.keys"),
        .edit_contract = CONTRACT("native.keys.toggle"),
        .create = create, .destroy = destroy, .unload = unload,
        .initialize = initialize, .prepare = prepare, .observe = observe,
        .release = release, .drop_state = drop_state,
    };
    return &module;
}
