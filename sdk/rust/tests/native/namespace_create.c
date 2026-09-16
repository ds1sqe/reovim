static uint64_t created_destroyed;
static void created_release(void *context) {
    Created *object = context;
    if (!--object->references) { free(object); created_destroyed++; }
}
static uint32_t created_provider(Created *object, rv_provider *output) {
    uint32_t result = fixture_create(output);
    if (result) return result;
    Counter *counter = output->context;
    counter->value = object->value;
    counter->shared = &object->value;
    counter->owner = object;
    counter->release_owner = created_release;
    object->references++;
    return RV_OK;
}
static uint64_t creations;
static int create_unknown, create_conflict;
static uint32_t create_name(void *context, rv_bytes name,
    const rv_creation *creation, rv_pending *output, rv_error *error) {
    Directory *directory = context;
    if (present(directory, name_slot(directory, name))) {
        error->code = RV_CONFLICT; return RV_CONFLICT;
    }
    if (!creation->keyed) {
        error->code = RV_UNSUPPORTED; return RV_UNSUPPORTED;
    }
    if (!name.len || name.len > sizeof(directory->added_name) ||
        directory->added_length || directory->root->created_generation == 255) {
        error->code = RV_PRESSURE; return RV_PRESSURE;
    }
    Listing *request = listing(directory);
    if (!request) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    Created *object = calloc(1, sizeof(*object));
    if (!object) {
        free(request);
        error->code = RV_PRESSURE;
        return RV_PRESSURE;
    }
    object->references = 1;
    object->key[0] = create_conflict ? 0xff : 0xfd;
    object->key[2] = create_conflict ? directory->generation :
        ++directory->root->created_generation;
    rv_provider child = {0};
    uint32_t result = created_provider(object, &child);
    if (!result) {
        result = creation->keyed(creation->context,
            (rv_bytes){object->key, sizeof(object->key)}, &child);
    }
    if (result) {
        if (child.context) {
            child.stop(child.context);
            child.destroy(child.context);
        }
        created_release(object);
        free(request);
        error->code = result;
        return result;
    }
    /* Staging owns the key/provider before the naming effect becomes visible. */
    directory->created = object;
    memcpy(directory->added_key, object->key, sizeof(object->key));
    memcpy(directory->added_name, name.ptr, name.len);
    directory->added_length = name.len;
    creations++;
    request->activity = RV_WRITE;
    request->unknown = create_unknown;
    return complete_listing(request, output);
}
uint64_t fixture_creations(void) { return creations; }
void fixture_create_unknown(void) { create_unknown = 1; }

void fixture_create_conflict(void) { create_conflict = 1; }

uint64_t fixture_created_destroyed(void) { return created_destroyed; }

void fixture_create_known(void) { create_unknown = 0; }
