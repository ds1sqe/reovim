#include "provider_export.c"

rv_state fixture_retain_state(const rv_state *state) {
    if (state->ops->bytes != sizeof(rv_state_ops)) abort();
    state->ops->retain(state->data);
    return *state;
}
void fixture_release_state(const rv_state *state) {
    state->ops->release(state->data);
}
uint32_t fixture_state_snapshot(const rv_state *state, rv_provider *output,
                                rv_error *error) {
    return state->ops->snapshot(state->data, output, error);
}
uint32_t fixture_rejected_snapshot(const rv_state *state, rv_error *error) {
    rv_provider output = {.bytes = 123, .context = error};
    uint32_t status = state->ops->snapshot(state->data, &output, error);
    if (!status || status != error->code) abort();
    if (output.bytes != 123 || output.context != error || output.interfaces ||
        output.interface_count || output.open || output.io ||
        output.namespace_ops || output.stop || output.destroy) abort();
    return status;
}
