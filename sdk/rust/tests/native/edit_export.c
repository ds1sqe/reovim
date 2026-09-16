#include "pending.c"

typedef struct { rv_state candidate; int reject; } Staging;
static uint64_t proposals;
static uint32_t propose(void *context, const rv_state *candidate) {
    Staging *staging = context;
    proposals++;
    if (staging->reject) return RV_DENIED;
    if (staging->candidate.ops) return RV_CONFLICT;
    candidate->ops->retain(candidate->data);
    staging->candidate = *candidate;
    return RV_OK;
}
static uint64_t snapshot_value(rv_state state) {
    rv_provider provider = {0};
    rv_error error = {0};
    if (state.ops->snapshot(state.data, &provider, &error)) abort();
    state.ops->release(state.data);
    if (!provider.io || provider.io->write || provider.io->read_modes != 2)
        abort();
    rv_read input = {1, 0, 8};
    rv_pending pending = {0};
    if (provider.io->read(provider.context, &input, &pending, &error)) abort();
    rv_wake wake = {0};
    if (pending.ops->poll(pending.context, wake) != RV_POLL_READY) abort();
    rv_outcome output = {0};
    if (pending.ops->take(pending.context, &output)) abort();
    const uint8_t *data = output.value.data.ptr;
    if (output.status || output.value.refs.len || output.value.data.len != 13 ||
        data[0] != 8 || data[1] || data[2] || data[3] || data[12] != 1) abort();
    uint64_t value = 0;
    for (unsigned i = 0; i < 8; i++) value |= (uint64_t)data[4+i] << (8*i);
    output.release(output.owner);
    if (pending.ops->finish(pending.context)) abort();
    if (provider.stop(provider.context)) abort();
    provider.destroy(provider.context);
    return value;
}
void fixture_edit(rv_provider *provider, const rv_state *before, uint32_t mode) {
    Staging staging = {.reject = mode == 2};
    rv_edit edit = {before->data, &staging, propose};
    const rv_interface *api = provider->interfaces;
    if (!api->edit || api->exec || api->control || api->factory) abort();
    uint8_t delta[] = {7,0,0,0,0,0,0,0};
    rv_args input = {.data = {delta, sizeof(delta)}};
    rv_pending pending = {0};
    rv_error error = {0};
    uint32_t result = api->edit->prepare(api->context, 7, &edit, &input,
                                        &pending, &error);
    if (mode == 1) {
        if (result != RV_DENIED || error.code != RV_DENIED ||
            pending.context || pending.ops || staging.candidate.ops) abort();
        return;
    }
    if (result) abort();
    rv_wake wake = {0};
    if (pending.ops->poll(pending.context, wake) != RV_POLL_READY) abort();
    rv_outcome output = {0};
    if (pending.ops->take(pending.context, &output)) abort();
    if (output.status != (mode == 2 ? RV_OUTCOME_FAILED : RV_OUTCOME_OK)) abort();
    if (mode == 2 && output.error.code != RV_DENIED) abort();
    if (pending.ops->finish(pending.context) != RV_BUSY) abort();
    output.release(output.owner);
    if (mode == 3 && pending.ops->finish(pending.context) != RV_BUSY) abort();
    if (pending.ops->finish(pending.context)) abort();
    if (mode != 2 && snapshot_value(staging.candidate) != 98) abort();
    if (mode == 2 && staging.candidate.ops) abort();
}
uint64_t fixture_proposals(void) { return proposals; }
