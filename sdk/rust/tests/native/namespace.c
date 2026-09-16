#include "operations.c"
#include <string.h>

typedef struct {
    uint64_t value, references;
    uint8_t key[3];
} Created;
static void created_release(void *);
static uint32_t created_provider(Created *, rv_provider *);

typedef struct Directory {
    struct Directory *root, *child;
    uint64_t generation, active, created_generation;
    Created *created;
    int nested, disk_removed, alias_removed;
    uint8_t added_name[64], added_key[3];
    size_t added_length;
} Directory;
typedef struct {
    Directory *directory;
    uint8_t data[192];
    size_t length;
    int taken, outstanding, unknown;
    uint32_t activity;
} Listing;
static uint64_t directories_destroyed, lookups, listings, materializations;
static Directory *latest;
static int materialize_unknown, lookup_unknown, nested_enabled;
static int flags_unknown, directory_empty, bad_flags;
static uint64_t flag_queries, removals, removed;
static int remove_unknown, link_unknown;
static uint64_t links, linked;
static uint32_t child_directory(Directory *, rv_provider *);
static void listing_release(void *context) {
    ((Listing *)context)->outstanding = 0;
}
static uint32_t listing_take(void *context, rv_outcome *output) {
    Listing *request = context;
    if (request->taken) return RV_CLOSED;
    *output = (rv_outcome){
        .status = request->unknown ? RV_OUTCOME_UNKNOWN : RV_OUTCOME_OK,
        .error = {.code = request->unknown ? RV_UNAVAILABLE : 0},
        .activity = request->activity,
        .value = {.data = {request->data,
            request->unknown ? 0 : request->length}},
        .owner = request, .release = listing_release
    };
    request->taken = request->outstanding = 1;
    return RV_OK;
}
static uint32_t listing_finish(void *context) {
    Listing *request = context;
    if (request->unknown || !request->taken || request->outstanding)
        return RV_BUSY;
    request->directory->active--;
    free(request);
    return RV_OK;
}
static uint32_t listing_reconcile(void *context) {
    Listing *request = context;
    if (!request->unknown || !request->taken) return RV_CLOSED;
    if (request->outstanding) return RV_BUSY;
    request->unknown = request->taken = 0;
    return RV_OK;
}
static const rv_pending_ops listing_ops = {
    sizeof(rv_pending_ops), poll, listing_take, cancel, listing_reconcile, listing_finish
};
static Listing *listing(Directory *directory) {
    Listing *request = calloc(1, sizeof(*request));
    if (request) {
        request->directory = directory;
        request->activity = RV_READ;
    }
    return request;
}
static uint32_t complete_listing(Listing *request, rv_pending *output) {
    request->directory->active++;
    *output = (rv_pending){request, &listing_ops};
    return RV_OK;
}
static void bytes(Listing *request, const uint8_t *data, size_t length) {
    for (unsigned i = 0; i < 4; i++)
        request->data[request->length++] = (uint8_t)(length >> (8 * i));
    memcpy(request->data + request->length, data, length);
    request->length += length;
}
static uint32_t lookup(void *context, rv_bytes name, rv_pending *output,
                       rv_error *error) {
    Directory *directory = context;
    lookups++;
    int branch = nested_enabled &&
        ((name.len == 6 && !memcmp(name.ptr, "branch", 6)) ||
         (name.len == 5 && !memcmp(name.ptr, "again", 5)));
    int disk = name.len == 4 && !memcmp(name.ptr, "disk", 4);
    int alias = name.len == 5 && !memcmp(name.ptr, "alias", 5);
    int added = directory->added_length &&
        name.len == directory->added_length &&
        !memcmp(name.ptr, directory->added_name, name.len);
    if (directory_empty || !(branch || added || (disk && !directory->disk_removed) ||
                             (alias && !directory->alias_removed))) {
        *error = (rv_error){.code = RV_UNAVAILABLE};
        return RV_UNAVAILABLE;
    }
    Listing *request = listing(directory);
    if (!request) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    uint8_t key[] = {0xff, 0, (uint8_t)directory->generation};
    request->unknown = lookup_unknown;
    request->activity = 0;
    if (branch) key[0] = 0xfe;
    if (added) memcpy(key, directory->added_key, sizeof(key));
    bytes(request, key, sizeof(key));
    return complete_listing(request, output);
}
static uint32_t list(void *context, uint64_t cursor, uint64_t maximum,
                     rv_pending *output, rv_error *error) {
    Directory *directory = context;
    listings++;
    if (cursor) { error->code = RV_INVALID; return RV_INVALID; }
    Listing *request = listing(directory);
    if (!request) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    if (directory_empty) {
        request->length = 12;
        return complete_listing(request, output);
    }
    request->data[0] = 0;
    request->length = 4;
    uint8_t key[] = {0xff, 0, (uint8_t)directory->generation};
    if (!directory->disk_removed) {
        request->data[0]++;
        bytes(request, (const uint8_t *)"disk", 4);
        bytes(request, key, sizeof(key));
    }
    if (!directory->alias_removed) {
        request->data[0]++;
        bytes(request, (const uint8_t *)"alias", 5);
        bytes(request, key, sizeof(key));
    }
    if (directory->added_length) {
        request->data[0]++;
        bytes(request, directory->added_name, directory->added_length);
        bytes(request, directory->added_key, sizeof(key));
    }
    if (nested_enabled) {
        request->data[0]++;
        key[0] = 0xfe;
        bytes(request, (const uint8_t *)"branch", 6);
        bytes(request, key, sizeof(key));
    }
    request->length += 8; /* Last page; calloc supplied canonical U64 zero. */
    if (request->length > maximum) {
        free(request);
        error->code = RV_PRESSURE;
        return RV_PRESSURE;
    }
    return complete_listing(request, output);
}
static uint32_t materialize(void *context, rv_bytes key,
    const rv_creation *creation, rv_pending *output, rv_error *error) {
    Directory *directory = context;
    materializations++;
    int created = key.len == 3 && directory->created &&
        !memcmp(key.ptr, directory->created->key, 3);
    if (!created && (directory->nested ||
        (directory->disk_removed && directory->alias_removed &&
         !directory->added_length) || key.len != 3 ||
        (key.ptr[0] != 0xff && !(nested_enabled && key.ptr[0] == 0xfe)) ||
        key.ptr[1] != 0 ||
        key.ptr[2] != directory->generation)) {
        error->code = RV_CONFLICT;
        return RV_CONFLICT;
    }
    Listing *request = listing(directory);
    if (!request) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    request->unknown = materialize_unknown;
    rv_provider child = {0};
    uint32_t result;
    if (created) result = created_provider(directory->created, &child);
    else if (key.ptr[0] == 0xfe) result = child_directory(directory, &child);
    else result = fixture_create(&child);
    if (!result) result = creation->resource(creation->context, &child);
    if (result) {
        if (child.context) {
            child.stop(child.context);
            child.destroy(child.context);
        }
        free(request);
        error->code = result;
        return result;
    }
    return complete_listing(request, output);
}
static uint32_t directory_stop(void *context) {
    return ((Directory *)context)->active ? RV_BUSY : RV_OK;
}
static void directory_destroy(void *context) {
    Directory *directory = context;
    if (directory->nested && directory->root->child == directory)
        directory->root->child = NULL;
    if (directory->created) created_release(directory->created);
    free(context);
    directories_destroyed++;
}
static uint32_t remove_name(void *context, rv_bytes name, rv_bytes key,
    rv_pending *output, rv_error *error) {
    Directory *directory = context;
    removals++;
    int disk = name.len == 4 && !memcmp(name.ptr, "disk", 4);
    int alias = name.len == 5 && !memcmp(name.ptr, "alias", 5);
    int added = directory->added_length &&
        name.len == directory->added_length &&
        !memcmp(name.ptr, directory->added_name, name.len);
    if (directory_empty || (!disk && !alias && !added) ||
        (disk && directory->disk_removed) ||
        (alias && directory->alias_removed) || key.len != 3 ||
        (added ? memcmp(key.ptr, directory->added_key, 3) :
         (key.ptr[0] != 0xff || key.ptr[1] ||
          key.ptr[2] != directory->generation))) {
        error->code = RV_CONFLICT;
        return RV_CONFLICT;
    }
    Listing *request = listing(directory);
    if (!request) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    if (disk) directory->disk_removed = 1;
    if (alias) directory->alias_removed = 1;
    if (added) {
        directory->added_length = 0;
        if (directory->created) {
            created_release(directory->created);
            directory->created = NULL;
        }
    }
    removed++;
    request->unknown = remove_unknown;
    request->activity = RV_WRITE;
    return complete_listing(request, output);
}
static uint32_t link_name(void *context, rv_bytes name, rv_bytes key,
    rv_pending *output, rv_error *error) {
    Directory *directory = context;
    links++;
    int disk = name.len == 4 && !memcmp(name.ptr, "disk", 4);
    int alias = name.len == 5 && !memcmp(name.ptr, "alias", 5);
    int branch = nested_enabled &&
        ((name.len == 6 && !memcmp(name.ptr, "branch", 6)) ||
         (name.len == 5 && !memcmp(name.ptr, "again", 5)));
    if (branch || (disk && !directory->disk_removed) ||
        (alias && !directory->alias_removed) ||
        (directory->added_length == name.len &&
         !memcmp(directory->added_name, name.ptr, name.len))) {
        error->code = RV_CONFLICT; return RV_CONFLICT;
    }
    if (key.len != 3 || key.ptr[0] != 0xff || key.ptr[1] ||
        key.ptr[2] != directory->generation) {
        error->code = RV_CONFLICT; return RV_CONFLICT;
    }
    if (name.len > sizeof(directory->added_name) ||
        (!disk && !alias && directory->added_length)) {
        error->code = RV_PRESSURE; return RV_PRESSURE;
    }
    Listing *request = listing(directory);
    if (!request) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    if (disk) directory->disk_removed = 0;
    else if (alias) directory->alias_removed = 0;
    else {
        memcpy(directory->added_name, name.ptr, name.len);
        directory->added_length = name.len;
        memcpy(directory->added_key, key.ptr, sizeof(directory->added_key));
    }
    linked++;
    request->unknown = link_unknown;
    request->activity = RV_WRITE;
    return complete_listing(request, output);
}
#include "namespace_rename.c"
#include "namespace_create.c"

static uint32_t flags(void *context, rv_pending *output, rv_error *error) {
    Listing *request = listing(context);
    flag_queries++;
    if (!request) { error->code = RV_PRESSURE; return RV_PRESSURE; }
    Directory *directory = context;
    int empty = directory_empty || (!nested_enabled &&
        directory->disk_removed && directory->alias_removed &&
        !directory->added_length);
    request->data[0] = bad_flags ? 2 : (empty ? 3 : 1);
    request->length = 4;
    request->unknown = flags_unknown;
    request->activity = 0;
    return complete_listing(request, output);
}
static const rv_namespace_ops namespace_ops = {
    .bytes = sizeof(rv_namespace_ops), .flags = flags, .lookup = lookup, .list = list,
    .materialize = materialize, .remove = remove_name, .link = link_name,
    .rename = rename_name, .create = create_name
};
uint32_t fixture_directory(rv_provider *output) {
    Directory *directory = calloc(1, sizeof(*directory));
    if (!directory) return RV_PRESSURE;
    directory->root = directory;
    directory->generation = 1;
    latest = directory;
    *output = (rv_provider){.bytes = sizeof(rv_provider), .context = directory,
        .namespace_ops = &namespace_ops, .stop = directory_stop,
        .destroy = directory_destroy};
    return RV_OK;
}
void fixture_replace(void) { latest->generation++; }
uint64_t fixture_directories_destroyed(void) { return directories_destroyed; }
uint64_t fixture_lookups(void) { return lookups; }

uint64_t fixture_listings(void) { return listings; }

uint64_t fixture_materializations(void) { return materializations; }

void fixture_materialize_unknown(void) { materialize_unknown = 1; }

static uint32_t child_directory(Directory *parent, rv_provider *output) {
    Directory *directory = calloc(1, sizeof(*directory));
    if (!directory) return RV_PRESSURE;
    directory->root = parent->root;
    directory->root->child = directory;
    directory->generation = parent->generation;
    directory->nested = 1;
    *output = (rv_provider){.bytes = sizeof(rv_provider), .context = directory,
        .namespace_ops = &namespace_ops, .stop = directory_stop,
        .destroy = directory_destroy};
    return RV_OK;
}
void fixture_nested(void) { nested_enabled = 1; }

void fixture_lookup_unknown(void) { lookup_unknown = 1; }

void fixture_empty(void) { directory_empty = 1; }
void fixture_flags_unknown(void) { flags_unknown = 1; }
uint64_t fixture_flag_queries(void) { return flag_queries; }

void fixture_bad_flags(void) { bad_flags = 1; }

uint64_t fixture_removals(void) { return removals; }
uint64_t fixture_removed(void) { return removed; }
void fixture_remove_unknown(void) { remove_unknown = 1; }
void fixture_restore(void) {
    latest->generation++;
    latest->disk_removed = latest->alias_removed = 0;
}

uint64_t fixture_links(void) { return links; }
uint64_t fixture_linked(void) { return linked; }
void fixture_link_unknown(void) { link_unknown = 1; }
