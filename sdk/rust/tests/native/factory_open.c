#define RV_FACTORY_OPEN
#include "factory.c"

typedef struct {
    Child *subject;
    int taken, outstanding, unknown, failed;
} Initial;
static uint32_t fail_mask, unknown_mask, omit_mask;
static uint64_t opens[3];

static uint32_t initial_poll(void *context, rv_wake wake) {
    (void)context; (void)wake;
    return RV_POLL_READY;
}
static void initial_release(void *context) {
    Initial *initial = context;
    assert(initial->outstanding);
    initial->outstanding = 0;
}
static uint32_t initial_take(void *context, rv_outcome *output) {
    Initial *initial = context;
    if (initial->taken) return RV_CLOSED;
    uint32_t status = initial->unknown ? RV_OUTCOME_UNKNOWN :
        (initial->failed ? RV_OUTCOME_FAILED : RV_OUTCOME_OK);
    *output = (rv_outcome){
        .status = status,
        .error = {.code = status == RV_OUTCOME_OK ? 0 : RV_UNAVAILABLE},
        .owner = initial, .release = initial_release
    };
    initial->taken = initial->outstanding = 1;
    return RV_OK;
}
static uint32_t initial_reconcile(void *context) {
    Initial *initial = context;
    if (!initial->unknown || !initial->taken) return RV_CLOSED;
    if (initial->outstanding) return RV_BUSY;
    initial->unknown = initial->taken = 0;
    return RV_OK;
}
static uint32_t initial_finish(void *context) {
    Initial *initial = context;
    if (!initial->taken || initial->outstanding || initial->unknown)
        return RV_BUSY;
    initial->subject->active--;
    free(initial);
    return RV_OK;
}
static const rv_pending_ops initial_ops = {
    sizeof(rv_pending_ops), initial_poll, initial_take, cancel,
    initial_reconcile, initial_finish
};
static uint32_t child_open(void *context, const rv_open *target,
    rv_pending *output, rv_error *error) {
    Child *subject = context;
    unsigned index = subject->value == 0xfeed1234 ? 0 :
        (unsigned)(subject->value - 0xabc000 + 1);
    assert(index < 3 && finished > 0);
    Initial *initial = calloc(1, sizeof(*initial));
    assert(initial);
    rv_provider provider = child_provider(0);
    Child *cursor = provider.context;
    cursor->parent = subject;
    cursor->value = subject->value + 0x100;
    provider.open = NULL;
    if (omit_mask & (1u << index)) provider.interface_count = 0;
    subject->active++;
    uint32_t status = target->stage(target->context, &provider);
    if (status) {
        dispose(provider);
        free(initial);
        error->code = status;
        return status;
    }
    subject->active++;
    initial->subject = subject;
    initial->failed = (fail_mask & (1u << index)) != 0;
    initial->unknown = (unknown_mask & (1u << index)) != 0;
    opens[index]++;
    *output = (rv_pending){initial, &initial_ops};
    return RV_OK;
}
static const rv_open_ops child_open_ops = {sizeof(rv_open_ops), child_open};
void fixture_fail_mask(uint32_t mask) { fail_mask = mask; }
void fixture_unknown_mask(uint32_t mask) { unknown_mask = mask; }
void fixture_omit_mask(uint32_t mask) { omit_mask = mask; }
uint64_t fixture_primary_opens(void) { return opens[0]; }
uint64_t fixture_first_opens(void) { return opens[1]; }
uint64_t fixture_second_opens(void) { return opens[2]; }
uint64_t fixture_opened_destroyed(void) { return opened_destroyed; }
