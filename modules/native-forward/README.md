# Native forwarding

A C Resource provider imports a granted Kernel connection through the public
ABI and submits counter operations. It has no Rust implementation dependency.

```sh
cc -std=c11 -Wall -Wextra -Werror -shared -fPIC \
  -I host/reference/include modules/native-forward/forward.c \
  -o /tmp/reovim-forward.so
cli=product/target/debug/reovim-cli
$cli 127.0.0.1:7878 write modules/catalog/counter kernel.module.activate 1
payload=$(python3 -c 'print(
    b"/tmp/reovim-forward.so\0modules/forward/value".hex())')
$cli 127.0.0.1:7878 write kernel/modules/load host.module.load-with 1 \
  "$payload" modules/counter/value # Grant this Resource to the module.
$cli 127.0.0.1:7878 write modules/forward/value native.forward 1 \
  0400000000000000 # Queue one addition of 4; do not repeat to collect it.
$cli 127.0.0.1:7878 read modules/forward/value native.forward 1
# Retry this read if Busy (error code 8); success returns a little-endian u64.
$cli 127.0.0.1:7878 write @ID kernel.module.unload 1
# Use the instance reference returned by loading.
```

The module retains its imported connection and pending request until collection
or unload. Loading with an unrelated Resource grant fails construction. The
final E2E checks this isolation, rejected-load scope reclamation and forwarding.
