#include "reovim.h"
#include <stdlib.h>

static void *forbidden(const void *declaration,
                       const struct rv_connection *kernel) {
    (void)declaration;
    (void)kernel;
    abort();
}
const struct rv_module *rv_module_entry(void) {
    static const struct rv_module module = {
        .version = { 9, 0, 0, 0 }, .size = sizeof(struct rv_module), .kind = 1,
        .create = forbidden,
    };
    return &module;
}
