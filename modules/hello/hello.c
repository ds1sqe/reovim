/* hello: the C example Driver. One rd, /module/hello/count, whose
 * bytes are a number in decimal: read gives it, write sets it.
 *
 *   cc -shared -fPIC -I kernel/abi/include hello.c -o hello.rvm
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "reovim.h"

static rv_rd count_rd;
static unsigned long count;

static rv_reply start(void *ctx, const rv_calls *k, rv_rd home) {
    rv_reply rd = k->register_(k->ctx, 0, home, "count");
    (void)ctx;
    if (rd.status != RV_OK)
        return rd;
    count_rd = rd.value;
    return k->report(k->ctx, rd.value, RV_READ | RV_WRITE, NULL, 0);
}

static rv_reply open_(void *ctx, const rv_calls *k, rv_rd rd, uint32_t rights) {
    (void)ctx; (void)k; (void)rights;
    return (rv_reply){ RV_OK, rd };
}

static rv_reply read(void *ctx, const rv_calls *k, uint64_t h, uint64_t at,
                     uint8_t *buf, size_t len) {
    char text[32];
    size_t n;
    (void)ctx; (void)k; (void)h;
    n = (size_t)snprintf(text, sizeof text, "%lu\n", count);
    if (at >= n)
        return (rv_reply){ RV_OK, 0 };
    n -= (size_t)at;
    if (n > len)
        n = len;
    memcpy(buf, text + at, n);
    return (rv_reply){ RV_OK, n };
}

static rv_reply write(void *ctx, const rv_calls *k, uint64_t h, uint64_t at,
                      rv_bytes buf) {
    char text[32];
    char *end;
    (void)ctx; (void)h; (void)at;
    if (buf.len == 0 || buf.len >= sizeof text)
        return (rv_reply){ RV_INVALID, 0 };
    memcpy(text, buf.ptr, buf.len);
    text[buf.len] = 0;
    count = strtoul(text, &end, 10);
    if (end == text)
        return (rv_reply){ RV_INVALID, 0 };
    k->changed(k->ctx, count_rd, RV_MODIFY);
    return (rv_reply){ RV_OK, buf.len };
}

static rv_reply close_(void *ctx, const rv_calls *k, uint64_t h) {
    (void)ctx; (void)k; (void)h;
    return (rv_reply){ RV_OK, 0 };
}

static const rv_driver driver = {
    NULL, NULL, open_, read, write, NULL, NULL, close_
};

static const rv_module module = {
    RV_MAGIC, RV_ABI, "hello", NULL, start, NULL, &driver, 1
};

const rv_module *rv_module_entry(void) { return &module; }
