static uint64_t renames, renamed;
static int rename_unknown;
static int integer(rv_bytes *input, uint32_t *value) {
    if (input->len < 4) return 0;
    *value = 0;
    for (unsigned i = 0; i < 4; i++)
        *value |= (uint32_t)input->ptr[i] << (8 * i);
    input->ptr += 4;
    input->len -= 4;
    return 1;
}
static int span(rv_bytes *input, rv_bytes *value) {
    uint32_t length;
    if (!integer(input, &length) || length > input->len) return 0;
    *value = (rv_bytes){input->ptr, length};
    input->ptr += length;
    input->len -= length;
    return 1;
}
static int name_slot(Directory *directory, rv_bytes name) {
    if (name.len == 4 && !memcmp(name.ptr, "disk", 4)) return 1;
    if (name.len == 5 && !memcmp(name.ptr, "alias", 5)) return 2;
    if (nested_enabled &&
        ((name.len == 6 && !memcmp(name.ptr, "branch", 6)) ||
         (name.len == 5 && !memcmp(name.ptr, "again", 5)))) return 4;
    if (directory->added_length && name.len == directory->added_length &&
        !memcmp(name.ptr, directory->added_name, name.len)) return 3;
    return 0;
}
static int present(Directory *directory, int slot) {
    return (slot == 1 && !directory->disk_removed) ||
        (slot == 2 && !directory->alias_removed) || slot == 3 || slot == 4;
}
static uint32_t rename_name(void *context, const rv_args *input,
    rv_pending *output, rv_error *error) {
    Directory *source = context, *destination = source->root;
    rv_bytes data = input->data, name, key, new_name, target = {0};
    uint32_t tag;
    renames++;
    if (input->refs.len || !span(&data, &name) || !span(&data, &key) ||
        !integer(&data, &tag) || tag > 1 ||
        (tag && !span(&data, &target)) || !span(&data, &new_name) || data.len) {
        error->code = RV_INVALID; return RV_INVALID;
    }
    if (tag) {
        destination = destination->child;
        if (!destination || target.len != 3 || target.ptr[0] != 0xfe ||
            target.ptr[1] || target.ptr[2] != destination->generation) {
            error->code = RV_CONFLICT; return RV_CONFLICT;
        }
    }
    int old_slot = name_slot(source, name);
    int new_slot = name_slot(destination, new_name);
    if (directory_empty || !present(source, old_slot) || old_slot == 4 ||
        present(destination, new_slot) || key.len != 3 ||
        key.ptr[0] != 0xff || key.ptr[1] ||
        key.ptr[2] != source->generation ||
        destination->generation != source->generation) {
        error->code = RV_CONFLICT; return RV_CONFLICT;
    }
    if (!new_name.len || new_name.len > sizeof(destination->added_name) ||
        (!new_slot && destination->added_length &&
         !(source == destination && old_slot == 3))) {
        error->code = RV_PRESSURE; return RV_PRESSURE;
    }
    Listing *request = listing(source);
    if (!request) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    /* Revalidation/allocation precedes this indivisible namespace mutation. */
    if (old_slot == 1) source->disk_removed = 1;
    if (old_slot == 2) source->alias_removed = 1;
    if (old_slot == 3) source->added_length = 0;
    if (new_slot == 1) destination->disk_removed = 0;
    else if (new_slot == 2) destination->alias_removed = 0;
    else {
        memcpy(destination->added_name, new_name.ptr, new_name.len);
        destination->added_length = new_name.len;
        memcpy(destination->added_key, key.ptr, sizeof(destination->added_key));
    }
    renamed++;
    request->unknown = rename_unknown;
    request->activity = RV_WRITE;
    return complete_listing(request, output);
}
uint64_t fixture_renames(void) { return renames; }
uint64_t fixture_renamed(void) { return renamed; }
void fixture_rename_unknown(void) { rename_unknown = 1; }
