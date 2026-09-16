#include "reovim.h"
#include <stdlib.h>

uint64_t fixture_observe(rv_pending *pending, uint32_t status,
                         uint32_t activity) {
    rv_wake wake = {0};
    if (pending->ops->poll(pending->context, wake) != 1) abort();
    rv_outcome output = {0};
    if (pending->ops->take(pending->context, &output) != RV_OK) abort();
    if (output.status != status || output.activity != activity ||
        output.value.data.len != 8 || output.value.refs.len) abort();
    uint64_t value = 0;
    for (unsigned i = 0; i < 8; i++)
        value |= (uint64_t)output.value.data.ptr[i] << (i * 8);
    if (pending->ops->finish(pending->context) != RV_BUSY) abort();
    output.release(output.owner);
    return value;
}
