#include "pending.c"

uint64_t fixture_control(rv_provider *provider, unsigned index) {
    const rv_interface *api = &provider->interfaces[index];
    rv_args input = {0};
    rv_pending pending = {0};
    rv_error error = {0};
    if (api->control->ioctl(api->context, 1, &input, &pending, &error)) abort();
    uint64_t value = fixture_observe(&pending, 0, 1);
    if (pending.ops->finish(pending.context)) abort();
    return value;
}
uint64_t fixture_execute(rv_provider *provider, unsigned index) {
    const rv_interface *api = &provider->interfaces[index];
    uint8_t bytes[] = {5, 0, 0, 0};
    rv_args input = {.data = {bytes, sizeof(bytes)}};
    rv_pending pending = {0};
    rv_error error = {0};
    if (api->exec->exec(api->context, &input, &pending, &error)) abort();
    uint64_t value = fixture_observe(&pending, 0, 2);
    if (pending.ops->finish(pending.context)) abort();
    return value;
}
void fixture_read(rv_provider *provider) {
    const rv_io_ops *io = provider->io;
    if (!io || io->read_modes != 2 || io->write_modes || io->write) abort();
    rv_read input = {1, 1, 3};
    rv_pending pending = {0};
    rv_error error = {0};
    if (io->read(provider->context, &input, &pending, &error)) abort();
    rv_wake wake = {0};
    if (pending.ops->poll(pending.context, wake) != 1) abort();
    rv_outcome output = {0};
    if (pending.ops->take(pending.context, &output)) abort();
    const uint8_t expected[] = {3, 0, 0, 0, 0xff, 'a', 'b', 1};
    if (output.status || output.activity != 1 || output.value.refs.len ||
        output.value.data.len != sizeof(expected)) abort();
    for (unsigned i = 0; i < sizeof(expected); i++)
        if (output.value.data.ptr[i] != expected[i]) abort();
    if (pending.ops->finish(pending.context) != RV_BUSY) abort();
    output.release(output.owner);
    if (pending.ops->finish(pending.context)) abort();
}
uint64_t fixture_write(rv_provider *provider) {
    const uint8_t bytes[] = {0, 0xff, 'a', 'b'};
    rv_write input = {0, 0, {bytes, sizeof(bytes)}};
    rv_pending pending = {0};
    rv_error error = {0};
    if (provider->io->write(provider->context, &input, &pending, &error)) abort();
    uint64_t value = fixture_observe(&pending, 0, 2);
    if (pending.ops->finish(pending.context)) abort();
    return value;
}
