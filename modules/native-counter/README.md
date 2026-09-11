# Native counter

An independently built C Resource module using module ABI 0.1.0. It imports the
public C header, without Rust types or a Kernel implementation dependency.

```sh
cc -std=c11 -Wall -Wextra -Werror -shared -fPIC \
  -I host/reference/include modules/native-counter/counter.c \
  -o /tmp/reovim-counter.so
```

On the running Linux server, write `host.module.load` v1 to
`kernel/modules/load`. Its payload is the UTF-8 library path, a NUL byte, then
the desired Resource path, encoded as hex by the shell. Loading runs in the
host; success returns the ready module instance reference.

```sh
payload=$(python3 -c 'print(
    b"/tmp/reovim-counter.so\0modules/native/value".hex())')
cli=product/target/debug/reovim-cli
$cli 127.0.0.1:7878 write kernel/modules/load host.module.load 1 "$payload"
$cli 127.0.0.1:7878 write modules/native/value native.counter 1 07
$cli 127.0.0.1:7878 write @ID kernel.module.unload 1
# Use the @ID returned by loading. Loading again creates fresh state.
```

`native.counter` v1 writes add one byte modulo 256; reads return the value as
an eight-byte little-endian integer. The host retains library code through
provider use and unload. The local control scope is trusted to select code
obeying the native ABI. The same entry point also loads [State Models](../native-keys/README.md).

The final E2E builds this library after server startup, loads/uses/unloads it,
then reloads it without rebuilding or restarting the server.
