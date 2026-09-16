/* Reuse the independent control/pending fixture, with a readonly declaration. */
#include "operations.c"

static const uint8_t state_read[] = {
    0x76,0x76,0x76,0x76,0x76,0x76,0x76,0x76,
    0x76,0x76,0x76,0x76,0x76,0x76,0x76,0x76,
    1,0,0,0,
    2,1,0,0,0,0,1,0,0,0,1,4,0,'r','e','a','d',0,4,0
};
typedef struct { uint64_t refs, value; uint32_t mode; } Revision;
static uint64_t state_refs, state_dead, snapshots;
static uint32_t cleanup_busy;
static void (*snapshot_hook)(void *);
static void *hook_context;
static void state_retain(const void *data) {
    Revision *state = (Revision *)data;
    state->refs++;
    state_refs++;
}
static void state_release(const void *data) {
    Revision *state = (Revision *)data;
    state_refs--;
    if (--state->refs == 0) {
        free(state);
        state_dead++;
    }
}
static void snapshot_release(void *data) { state_release(data); }
static uint32_t snapshot_stop(void *data) {
    return cleanup_busy ? RV_BUSY : stop(data);
}
static uint32_t state_snapshot(const void *data, rv_provider *out,
                               rv_error *error) {
    Revision *state = (Revision *)data;
    snapshots++;
    if (snapshot_hook) {
        void (*hook)(void *) = snapshot_hook;
        snapshot_hook = NULL;
        hook(hook_context);
    }
    if (state->mode == 1) {
        *error = (rv_error){.code = RV_PRESSURE};
        return RV_PRESSURE;
    }
    uint32_t status = fixture_create(out);
    if (status != RV_OK) {
        *error = (rv_error){.code = status};
        return status;
    }
    Counter *counter = out->context;
    counter->shared = &state->value;
    counter->owner = state;
    counter->release_owner = snapshot_release;
    counter->interface.declaration = (rv_bytes){state_read, sizeof(state_read)};
    counter->interface.exec = NULL;
    state_retain(state);
    out->stop = snapshot_stop;
    if (state->mode == 2) counter->interface.declaration.len = 0;
    return RV_OK;
}
static const rv_state_ops state_ops = {
    sizeof(rv_state_ops), state_retain, state_release, state_snapshot
};
uint32_t fixture_state(uint32_t mode, rv_state *out) {
    Revision *state = calloc(1, sizeof(*state));
    if (!state) return RV_PRESSURE;
    *state = (Revision){1, 91, mode};
    state_refs++;
    *out = (rv_state){state, &state_ops, 17};
    return RV_OK;
}
void fixture_state_mode(const rv_state *raw, uint32_t mode) {
    ((Revision *)raw->data)->mode = mode;
}
void fixture_cleanup_busy(uint32_t busy) { cleanup_busy = busy; }
uint64_t fixture_state_refs(void) { return state_refs; }
uint64_t fixture_state_dead(void) { return state_dead; }
uint64_t fixture_snapshots(void) { return snapshots; }

void fixture_snapshot_hook(void (*hook)(void *), void *context) {
    snapshot_hook = hook;
    hook_context = context;
}
