#include "reovim.h"
#include <stdlib.h>

static void *forbidden(const ReovimConnection *kernel) {
    (void)kernel;
    abort();
}
const ReovimModule *reovim_module_v2(void) {
    static const ReovimModule module = {
        .version = 1, .size = sizeof(ReovimModule), .kind = 1,
        .create = forbidden,
    };
    return &module;
}
