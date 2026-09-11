# Native keyed-state model

An independent C State Model using module ABI 0.1.0 and immutable native states.
It defines boolean values at semantic keys 17 and 9001; keys are not offsets.

```sh
cc -std=c11 -Wall -Wextra -Werror -shared -fPIC \
  -I host/reference/include modules/native-keys/keys.c -o /tmp/reovim-keys.so
```

Load through `kernel/modules/load` with the same payload as
[native-counter](../native-counter/README.md), selecting `models/native-keys`
as the export path. The host reads the provider kind from the ABI descriptor.

| Contract v1 | Input | Result |
| --- | --- | --- |
| `native.keys.create` | Empty | Buffer with 17=true, 9001=false |
| `native.keys.toggle` | Common edit currentness + u64 key | New Buffer state |
| `native.keys` | Empty | Common observation + `(u64 key, u8 bool)` records |

All integers are little-endian. Common edit/observation encoding comes from
Kernel Interface. History Undo restores retained immutable state; it does
not call the module to reverse the operation. Live Buffers and History keep
native code loaded. Explicit Buffer close releases those states before unload.

`scripts/e2e.py` builds this module after server startup and verifies the
complete edit/Undo/close/unload path through the separate CLI.
