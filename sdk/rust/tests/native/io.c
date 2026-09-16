#include "reovim.h"
#include <assert.h>
#include <stdlib.h>
#include <string.h>

typedef struct { uint8_t data[8]; size_t size, active; } Device;
typedef struct {
    Device *device;
    uint8_t data[13];
    size_t size;
    int taken, outstanding;
    uint32_t activity;
} Request;
static uint64_t destroyed, reads, writes;
static uint32_t poll(void *context, rv_wake wake) {
    (void)context; (void)wake;
    return RV_POLL_READY;
}
static void release(void *context) {
    ((Request *)context)->outstanding = 0;
}
static uint32_t take(void *context, rv_outcome *output) {
    Request *request = context;
    if (request->taken) return RV_CLOSED;
    *output = (rv_outcome){
        .status = RV_OUTCOME_OK, .activity = request->activity,
        .value = {.data = {request->data, request->size}},
        .owner = request, .release = release
    };
    request->taken = request->outstanding = 1;
    return RV_OK;
}
static uint32_t cancel(void *context) {
    (void)context;
    return RV_CANCEL_TOO_LATE;
}
static uint32_t reconcile(void *context) {
    (void)context;
    return RV_UNSUPPORTED;
}
static uint32_t finish(void *context) {
    Request *request = context;
    if (!request->taken || request->outstanding) return RV_BUSY;
    request->device->active--;
    free(request);
    return RV_OK;
}
static const rv_pending_ops pending_ops = {
    sizeof(rv_pending_ops), poll, take, cancel, reconcile, finish
};
static Request *allocate(Device *device, rv_error *error) {
    Request *request = calloc(1, sizeof(*request));
    if (!request) {
        *error = (rv_error){.code = RV_PRESSURE};
        return NULL;
    }
    request->device = device;
    device->active++;
    return request;
}
static uint32_t read_bytes(void *context, const rv_read *input,
                          rv_pending *output, rv_error *error) {
    Device *device = context;
    assert(input->positional == 1 && input->maximum <= 8);
    Request *request = allocate(device, error);
    if (!request) return RV_PRESSURE;
    size_t start = input->offset < device->size ? input->offset : device->size;
    size_t count = device->size - start;
    if (count > input->maximum) count = input->maximum;
    request->data[0] = (uint8_t)count;
    memcpy(request->data + 4, device->data + start, count);
    request->data[count + 4] = start + count == device->size;
    request->size = count + 5;
    request->activity = RV_READ;
    *output = (rv_pending){request, &pending_ops};
    reads++;
    return RV_OK;
}
static uint32_t write_bytes(void *context, const rv_write *input,
                           rv_pending *output, rv_error *error) {
    Device *device = context;
    assert(input->positional == 0 && input->offset == 0);
    assert(input->data.len <= sizeof(device->data));
    Request *request = allocate(device, error);
    if (!request) return RV_PRESSURE;
    memcpy(device->data, input->data.ptr, input->data.len);
    device->size = input->data.len;
    request->data[0] = (uint8_t)input->data.len;
    request->size = 8;
    request->activity = RV_WRITE;
    *output = (rv_pending){request, &pending_ops};
    writes++;
    return RV_OK;
}
static const rv_io_ops io = {
    .bytes = sizeof(rv_io_ops), .read_modes = 2, .write_modes = 1,
    .alignment = 1, .transfer_limit = 8,
    .read = read_bytes, .write = write_bytes
};
static uint32_t stop(void *context) {
    return ((Device *)context)->active ? RV_BUSY : RV_OK;
}
static void destroy(void *context) { free(context); destroyed++; }
uint32_t fixture_create(rv_provider *output) {
    Device *device = calloc(1, sizeof(*device));
    if (!device) return RV_PRESSURE;
    *output = (rv_provider){
        .bytes = sizeof(rv_provider), .context = device, .io = &io,
        .stop = stop, .destroy = destroy
    };
    return RV_OK;
}
uint64_t fixture_destroyed(void) { return destroyed; }
uint64_t fixture_reads(void) { return reads; }
uint64_t fixture_writes(void) { return writes; }
