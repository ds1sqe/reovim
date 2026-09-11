# Kernel

Resource access, editing state and module execution. Clients own input,
view layout, rendering and user-facing output.

| Directory | Owns |
| --- | --- |
| [interface](interface/README.md) | Public contracts used by implementations and consumers |
| [runtime](runtime/README.md) | Resource ownership, execution and atomic state installation |

```text
runtime -> interface <- modules and callers
                 ^
             ABI adapters
```

Assembly selects implementations and grants access. Ordinary callers receive
the Interface's `Connection`; they do not import Runtime. Both crates use
`no_std + alloc`.

Keep implementation helpers private to their owner. Reusable data structures
belong in `lib/ds`, macros in `lib/macro`, and other shared utilities in an
appropriately named `lib/` directory. Add these only when a consumer needs
them; do not introduce another generic Kernel layer.
