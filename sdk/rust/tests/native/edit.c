/* Independent immutable revision and snapshot implementation. */
#include "state.c"

static const uint8_t edit_declaration[] = {
    0x78,0x78,0x78,0x78,0x78,0x78,0x78,0x78,
    0x78,0x78,0x78,0x78,0x78,0x78,0x78,0x78,
    1,0,0,0,
    2,7,0,0,0,1,2,0,0,0,2,4,0,'m','o','v','e',4,0,0
};
typedef struct { rv_interface interface; uint64_t active; } Editor;
typedef struct {
    Editor *editor;
    const rv_edit *edit;
    uint64_t delta;
    int taken, outstanding, cancelled;
} EditRequest;
static uint64_t prepares, editor_dead, last_before;
static uint32_t duplicate, last_proposal;

static void edit_release(void *context) {
    ((EditRequest *)context)->outstanding = 0;
}
static uint32_t edit_take(void *context, rv_outcome *output) {
    EditRequest *request = context;
    if (request->taken) return RV_CLOSED;
    uint32_t status = RV_CANCELLED;
    if (!request->cancelled) {
        rv_state candidate;
        status = fixture_state(0, &candidate);
        if (status != RV_OK) return status;
        Revision *next = (Revision *)candidate.data;
        const Revision *before = request->edit->before;
        next->value = before->value + request->delta;
        status = request->edit->propose(request->edit->context, &candidate);
        if (duplicate && status == RV_OK)
            last_proposal = request->edit->propose(
                request->edit->context, &candidate);
        else last_proposal = status;
        candidate.ops->release(candidate.data);
    }
    *output = (rv_outcome){
        .status = status == RV_OK ? RV_OUTCOME_OK : RV_OUTCOME_FAILED,
        .error = {.code = status},
        .owner = request, .release = edit_release
    };
    request->taken = request->outstanding = 1;
    return RV_OK;
}
static uint32_t edit_cancel(void *context) {
    EditRequest *request = context;
    if (request->taken) return RV_CANCEL_TOO_LATE;
    request->cancelled = 1;
    return RV_CANCEL_ACCEPTED;
}
static uint32_t edit_reconcile(void *context) {
    (void)context;
    return RV_UNSUPPORTED;
}
static uint32_t edit_finish(void *context) {
    EditRequest *request = context;
    if (!request->taken || request->outstanding) return RV_BUSY;
    last_before = ((const Revision *)request->edit->before)->value;
    request->editor->active--;
    free(request);
    return RV_OK;
}
static const rv_pending_ops edit_pending = {
    sizeof(rv_pending_ops), poll, edit_take, edit_cancel,
    edit_reconcile, edit_finish
};
static uint32_t edit_prepare(void *context, uint32_t command,
    const rv_edit *edit, const rv_args *input, rv_pending *out,
    rv_error *error) {
    if (command != 7 || input->data.len != 8 || input->refs.len) {
        *error = (rv_error){.code = RV_INVALID};
        return RV_INVALID;
    }
    EditRequest *request = calloc(1, sizeof(*request));
    if (!request) {
        *error = (rv_error){.code = RV_PRESSURE};
        return RV_PRESSURE;
    }
    request->editor = context;
    request->edit = edit;
    for (unsigned i = 0; i < 8; i++)
        request->delta |= (uint64_t)input->data.ptr[i] << (8 * i);
    request->editor->active++;
    prepares++;
    *out = (rv_pending){request, &edit_pending};
    return RV_OK;
}
static const rv_edit_ops edit_ops = {
    sizeof(rv_edit_ops), edit_prepare, NULL
};
static uint32_t editor_stop(void *context) {
    return ((Editor *)context)->active ? RV_BUSY : RV_OK;
}
static void editor_destroy(void *context) {
    free(context);
    editor_dead++;
}
uint32_t fixture_state_init(rv_state_init *output) {
    rv_state initial;
    uint32_t status = fixture_state(0, &initial);
    if (status != RV_OK) return status;
    Editor *editor = calloc(1, sizeof(*editor));
    if (!editor) {
        initial.ops->release(initial.data);
        return RV_PRESSURE;
    }
    editor->interface = (rv_interface){
        .bytes = sizeof(rv_interface), .context = editor,
        .declaration = {edit_declaration, sizeof(edit_declaration)},
        .edit = &edit_ops
    };
    *output = (rv_state_init){
        .initial = initial,
        .implementation = {
            .bytes = sizeof(rv_provider), .context = editor,
            .interfaces = &editor->interface, .interface_count = 1,
            .stop = editor_stop, .destroy = editor_destroy
        }
    };
    return RV_OK;
}
void fixture_duplicate(uint32_t enabled) { duplicate = enabled; }
uint64_t fixture_prepares(void) { return prepares; }
uint64_t fixture_editor_dead(void) { return editor_dead; }
uint64_t fixture_last_before(void) { return last_before; }
uint64_t fixture_last_proposal(void) { return last_proposal; }
