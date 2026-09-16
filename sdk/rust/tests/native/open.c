#include "io.c"

typedef struct {
    Device *device;
    size_t cursor, active;
    rv_interface interface;
} Cursor;
typedef struct { Request result; Cursor *cursor; } Transfer;
typedef struct { Request result; int unknown, failed; } Opening;
static uint64_t opens, cursors_destroyed;
static int open_unknown;
static int open_failed;
static int stop_busy;

static uint32_t cursor_finish(void *context) {
    Transfer *transfer = context;
    if (!transfer->result.taken || transfer->result.outstanding) return RV_BUSY;
    transfer->cursor->active--;
    return finish(context);
}
static const rv_pending_ops transfer_ops = {
    sizeof(rv_pending_ops), poll, take, cancel, reconcile, cursor_finish
};
static uint32_t cursor_read(void *context, const rv_read *input,
    rv_pending *output, rv_error *error) {
    Cursor *cursor = context;
    Transfer *transfer = calloc(1, sizeof(*transfer));
    if (!transfer) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    size_t start = input->positional ? input->offset : cursor->cursor;
    if (start > cursor->device->size) start = cursor->device->size;
    size_t count = cursor->device->size - start;
    if (count > input->maximum) count = input->maximum;
    transfer->cursor = cursor;
    Request *result = &transfer->result;
    result->device = cursor->device;
    result->data[0] = count;
    memcpy(result->data + 4, cursor->device->data + start, count);
    result->data[4 + count] = start + count == cursor->device->size;
    result->size = count + 5;
    result->activity = RV_READ;
    if (!input->positional) cursor->cursor = start + count;
    cursor->active++;
    cursor->device->active++;
    *output = (rv_pending){transfer, &transfer_ops};
    return RV_OK;
}
static uint32_t cursor_stop(void *context) {
    return stop_busy || ((Cursor *)context)->active ? RV_BUSY : RV_OK;
}
static void cursor_destroy(void *context) { free(context); cursors_destroyed++; }
static const rv_io_ops cursor_io = {
    .bytes = sizeof(rv_io_ops), .read_modes = 3, .alignment = 1,
    .transfer_limit = 8, .read = cursor_read
};
static const uint8_t cursor_declaration[] = {
    0x74,0x74,0x74,0x74,0x74,0x74,0x74,0x74,
    0x74,0x74,0x74,0x74,0x74,0x74,0x74,0x74,
    1,0,0,0,
    2, 1,0,0,0, 0, 1,0,0,0, 1,
    8,0,'p','o','s','i','t','i','o','n', 0,4,0
};
static uint32_t cursor_position(void *context, uint32_t command,
    const rv_args *input, rv_pending *output, rv_error *error) {
    if (command != 1 || input->data.len || input->refs.len) {
        error->code = RV_INVALID; return RV_INVALID;
    }
    Cursor *cursor = context;
    Transfer *transfer = calloc(1, sizeof(*transfer));
    if (!transfer) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    transfer->cursor = cursor;
    transfer->result.device = cursor->device;
    transfer->result.size = 8;
    transfer->result.activity = RV_READ;
    uint64_t position = cursor->cursor;
    for (size_t i = 0; i < 8; ++i) transfer->result.data[i] = position >> (8 * i);
    cursor->active++;
    cursor->device->active++;
    *output = (rv_pending){transfer, &transfer_ops};
    return RV_OK;
}
static uint32_t position_offer(void *context, uint32_t command,
    const rv_args *input, rv_pending *output, rv_error *error) {
    (void)context; (void)command; (void)input; (void)output;
    error->code = RV_UNSUPPORTED; return RV_UNSUPPORTED;
}
static const rv_control_ops position_ops = {sizeof(rv_control_ops), cursor_position};
static const rv_control_ops position_offer_ops = {sizeof(rv_control_ops), position_offer};
static const rv_interface offered_interface = {
    .bytes = sizeof(rv_interface),
    .declaration = {cursor_declaration, sizeof(cursor_declaration)},
    .control = &position_offer_ops
};
static uint32_t opening_take(void *context, rv_outcome *output) {
    Opening *opening = context;
    uint32_t status = take(context, output);
    if (!status && opening->unknown) {
        output->status = RV_OUTCOME_UNKNOWN;
        output->error.code = RV_UNAVAILABLE;
    }
    if (!status && opening->failed) {
        output->status = RV_OUTCOME_FAILED;
        output->error.code = RV_UNAVAILABLE;
    }
    return status;
}
static uint32_t opening_reconcile(void *context) {
    Opening *opening = context;
    if (!opening->unknown || !opening->result.taken) return RV_CLOSED;
    if (opening->result.outstanding) return RV_BUSY;
    opening->unknown = opening->result.taken = 0;
    return RV_OK;
}
static uint32_t opening_finish(void *context) {
    if (((Opening *)context)->unknown) return RV_BUSY;
    return finish(context);
}
static const rv_pending_ops opening_ops = {
    sizeof(rv_pending_ops), poll, opening_take, cancel,
    opening_reconcile, opening_finish
};
static uint32_t opened(void *context, const rv_open *target,
    rv_pending *output, rv_error *error) {
    if (!target || !target->stage) { error->code = RV_INVALID; return RV_INVALID; }
    Opening *opening = calloc(1, sizeof(*opening));
    Cursor *cursor = calloc(1, sizeof(*cursor));
    if (!opening || !cursor) {
        free(opening); free(cursor);
        error->code = RV_PRESSURE; return RV_PRESSURE;
    }
    cursor->device = context;
    cursor->interface = offered_interface;
    cursor->interface.context = cursor;
    cursor->interface.control = &position_ops;
    rv_provider implementation = {
        .bytes = sizeof(rv_provider), .context = cursor, .io = &cursor_io,
        .interfaces = &cursor->interface, .interface_count = 1,
        .stop = cursor_stop, .destroy = cursor_destroy
    };
    uint32_t result = target->stage(target->context, &implementation);
    if (result) {
        cursor_destroy(cursor);
        free(opening);
        error->code = result;
        return result;
    }
    opens++;
    opening->result.device = context;
    opening->result.device->active++;
    opening->unknown = open_unknown;
    opening->failed = open_failed;
    *output = (rv_pending){opening, &opening_ops};
    return RV_OK;
}
static const rv_open_ops open_ops = {sizeof(rv_open_ops), opened};
uint32_t fixture_open_subject(rv_provider *output) {
    uint32_t result = fixture_create(output);
    if (result) return result;
    Device *device = output->context;
    memcpy(device->data, "abcd", 4);
    device->size = 4;
    output->io = NULL;
    output->open = &open_ops;
    output->interfaces = &offered_interface;
    output->interface_count = 1;
    return RV_OK;
}
uint64_t fixture_opens(void) { return opens; }
uint64_t fixture_cursors_destroyed(void) { return cursors_destroyed; }
void fixture_open_unknown(void) { open_unknown = 1; }
void fixture_open_known(void) { open_unknown = 0; }
void fixture_open_failed(void) { open_failed = 1; }
void fixture_open_ready(void) { open_failed = 0; }
void fixture_stop_busy(void) { stop_busy = 1; }
void fixture_stop_ready(void) { stop_busy = 0; }
static uint32_t subject_read(void *context, const rv_read *input,
    rv_pending *output, rv_error *error) {
    (void)context; (void)input; (void)output;
    error->code = RV_UNSUPPORTED;
    return RV_UNSUPPORTED;
}
static const rv_io_ops offered_io = {
    .bytes = sizeof(rv_io_ops), .read_modes = 3, .alignment = 1,
    .transfer_limit = 8, .read = subject_read
};
uint32_t fixture_open_offered(rv_provider *output) {
    uint32_t result = fixture_open_subject(output);
    if (!result) output->io = &offered_io;
    return result;
}
