#include "reovim.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>

/* Record(sender U32, payload Bytes(8)); metadata is not a host packet enum. */
static const uint8_t packet_type[] = {
    9,2,0, 6,0,'s','e','n','d','e','r',3,
    7,0,'p','a','y','l','o','a','d',7,8,0,0,0
};
typedef struct { unsigned next, active; int stopped; } Device;
typedef struct {
    Device *device;
    uint8_t data[40];
    size_t size;
    uint32_t activity;
    int taken, outstanding, unknown;
    rv_wake wake;
    uint64_t refs[1];
    size_t ref_count;
} Request;
static uint64_t received, sent, destroyed, last_sender, last_size;
static uint8_t last_data[8];
static int uncertain;
static unsigned fault;
static int receive_wait;
static Request *waiting[8];
#if defined(RV_PACKET_REFERENCES) || defined(RV_RECEIVE_REFERENCES)
static const uint8_t reference_type[] = {
    12, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, 1,0,0,0
};
static uint64_t last_reference;
#endif
static uint32_t poll(void *context, rv_wake wake) {
    Request *request = context;
    if (!receive_wait || request->activity != RV_READ) return RV_POLL_READY;
    if (request->wake.context) request->wake.release(request->wake.context);
    request->wake = wake;
    wake.retain(wake.context);
    for (unsigned i = 0; i < 8; ++i) {
        if (waiting[i] == request) return RV_POLL_PENDING;
        if (!waiting[i]) {
            waiting[i] = request;
            return RV_POLL_PENDING;
        }
    }
    assert(0 && "fixture waiter capacity exhausted");
    return RV_POLL_PENDING;
}
static void release(void *context) {
    Request *request = context;
    assert(request->outstanding);
    request->outstanding = 0;
}
static uint32_t take(void *context, rv_outcome *output) {
    Request *request = context;
    if (request->taken) return RV_CLOSED;
    *output = (rv_outcome){
        .status = request->unknown ? RV_OUTCOME_UNKNOWN : RV_OUTCOME_OK,
        .error = {.code = request->unknown ? RV_UNAVAILABLE : 0},
        .activity = request->activity,
        .value = {
            .data = {request->data, request->unknown ? 0 : request->size},
            .refs = {request->refs, request->unknown ? 0 : request->ref_count}
        },
        .owner = request, .release = release
    };
    if (request->activity == RV_WRITE && fault == 1)
        output->activity = RV_READ;
    if (request->activity == RV_WRITE && fault == 2)
        output->value.data.len = 7;
    if (request->activity == RV_WRITE && fault == 3) {
        output->status = RV_OUTCOME_UNKNOWN;
        output->error.code = RV_UNAVAILABLE;
        output->value.data.len = 8;
    }
    if (request->activity == RV_READ && fault == 4)
        request->data[0] = 7; /* Undeclared result alternative. */
    if (request->activity == RV_READ && fault == 5)
        request->data[12] = 1; /* First item is empty, but claims truncation. */
    request->taken = request->outstanding = 1;
    return RV_OK;
}
static uint32_t cancel(void *context) {
    (void)context;
    return RV_CANCEL_TOO_LATE;
}
static uint32_t reconcile(void *context) {
    Request *request = context;
    if (!request->unknown || !request->taken) return RV_CLOSED;
    if (request->outstanding) return RV_BUSY;
    request->unknown = request->taken = 0;
    return RV_OK;
}
static uint32_t finish(void *context) {
    Request *request = context;
    if (!request->taken || request->unknown || request->outstanding)
        return RV_BUSY;
    if (request->wake.context) request->wake.release(request->wake.context);
    for (unsigned i = 0; i < 8; ++i)
        if (waiting[i] == request) waiting[i] = NULL;
    request->device->active--;
    free(request);
    return RV_OK;
}
static const rv_pending_ops pending_ops = {
    sizeof(rv_pending_ops), poll, take, cancel, reconcile, finish
};
static uint64_t number(const uint8_t *bytes, unsigned size) {
    uint64_t value = 0;
    for (unsigned i = 0; i < size; ++i) value |= (uint64_t)bytes[i] << (8*i);
    return value;
}
static void encode(uint8_t *data, uint64_t value, unsigned size) {
    for (unsigned i = 0; i < size; ++i) data[i] = value >> (8*i);
}
static Request *request(Device *device, rv_pending *output, uint32_t activity) {
    Request *pending = calloc(1, sizeof(*pending));
    assert(pending);
    pending->device = device;
    pending->activity = activity;
    pending->unknown = uncertain;
    device->active++;
    *output = (rv_pending){pending, &pending_ops};
    return pending;
}
static uint32_t receive(void *context, const rv_args *input,
    rv_pending *output, rv_error *error) {
    assert(input->data.len == 12 && input->refs.len == 0);
    Device *device = context;
    uint64_t maximum = number(input->data.ptr, 8);
    uint32_t oversize = (uint32_t)number(input->data.ptr + 8, 4);
    assert(oversize <= 2);
    size_t size = device->next == 1 ? 3 : (device->next == 2 ? 1 : 0);
    if (maximum < size && oversize == 0) {
        error->code = RV_PRESSURE;
        return RV_PRESSURE;
    }
    Request *pending = request(device, output, RV_READ);
#ifdef RV_RECEIVE_REFERENCES
    assert(last_reference != 0);
    pending->refs[0] = last_reference;
    pending->ref_count = 1;
    pending->size = 13; /* Item, Ref(0), false, original None. */
    received++;
    return RV_OK;
#endif
    if (device->next >= 3) {
        pending->data[0] = 2; /* End. */
        pending->size = 4;
        return RV_OK;
    }
    unsigned sequence = device->next++;
    received++;
    if (maximum < size && oversize == 2) {
        pending->data[0] = 1; /* Dropped, original Some(size). */
        pending->data[4] = 1;
        encode(pending->data + 8, size, 8);
        pending->size = 16;
        return RV_OK;
    }
    size_t count = maximum < size ? (size_t)maximum : size;
    static const uint8_t payload[] = {'a', 0xff, 'b'};
    encode(pending->data + 4, 41 + sequence, 4);
    encode(pending->data + 8, count, 4);
    memcpy(pending->data + 12, payload, count);
    pending->data[12 + count] = count != size;
    pending->data[13 + count] = 1; /* original Some(size). */
    encode(pending->data + 17 + count, size, 8);
    pending->size = 25 + count;
    return RV_OK;
}
static uint32_t send(void *context, const rv_args *input,
    rv_pending *output, rv_error *error) {
    (void)error;
#ifdef RV_PACKET_REFERENCES
    assert(input->data.len == 4 && number(input->data.ptr, 4) == 0);
    assert(input->refs.len == 1 && input->refs.ptr[0] != 0);
    last_reference = input->refs.ptr[0];
    size_t size = 0;
#else
    assert(input->data.len >= 8 && input->refs.len == 0);
    size_t size = (size_t)number(input->data.ptr + 4, 4);
    assert(size <= 8 && input->data.len == 8 + size);
    last_sender = number(input->data.ptr, 4);
    last_size = size;
    memcpy(last_data, input->data.ptr + 8, size);
#endif
    sent++;
    Request *pending = request(context, output, RV_WRITE);
    encode(pending->data, size, 8);
    pending->size = 8;
    return RV_OK;
}
static const rv_io_ops io = {
    .bytes = sizeof(rv_io_ops), .alignment = 1, .transfer_limit = 8,
#ifdef RV_RECEIVE_REFERENCES
    .receive_type = {reference_type, sizeof(reference_type)},
#else
    .receive_type = {packet_type, sizeof(packet_type)},
#endif
#ifdef RV_PACKET_REFERENCES
    .send_type = {reference_type, sizeof(reference_type)},
#else
    .send_type = {packet_type, sizeof(packet_type)},
#endif
    .receive = receive, .send = send
};
static uint32_t stop(void *context) {
    Device *device = context;
    if (device->active) return RV_BUSY;
    device->stopped = 1;
    return RV_OK;
}
static void destroy(void *context) {
    Device *device = context;
    assert(device->stopped && !device->active);
    destroyed++;
    free(device);
}
uint32_t fixture_create(rv_provider *output) {
    Device *device = calloc(1, sizeof(*device));
    assert(device);
    *output = (rv_provider){
        .bytes = sizeof(rv_provider), .context = device, .io = &io,
        .stop = stop, .destroy = destroy
    };
    return RV_OK;
}
void fixture_uncertain(void) { uncertain = 1; }
uint64_t fixture_received(void) { return received; }
uint64_t fixture_sent(void) { return sent; }
uint64_t fixture_destroyed(void) { return destroyed; }
uint64_t fixture_sender(void) { return last_sender; }
uint64_t fixture_size(void) { return last_size; }
uint64_t fixture_byte(uint32_t index) {
    assert(index < last_size);
    return last_data[index];
}

void fixture_fault(uint32_t value) { fault = value; }
#ifdef RV_PACKET_REFERENCES
uint64_t fixture_reference(void) { return last_reference; }
#endif

void fixture_wait(void) { receive_wait = 1; }
void fixture_ready(void) {
    receive_wait = 0;
    for (unsigned i = 0; i < 8; ++i) {
        if (!waiting[i]) continue;
        Request *request = waiting[i];
        waiting[i] = NULL;
        request->wake.wake(request->wake.context);
        request->wake.release(request->wake.context);
        request->wake = (rv_wake){0};
    }
}

#ifdef RV_RECEIVE_REFERENCES
void fixture_set_reference(uint64_t token) { last_reference = token; }
#endif
