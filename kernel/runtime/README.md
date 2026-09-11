# Kernel Runtime

Implements the [Interface](../interface/README.md). Owns Resource names and
grants, requests, Buffer/History, retained Session context and module lifetimes.

```rust
let mut runtime = Runtime::new(1, Limits::default())?;
runtime.start()?;
let source = runtime.register("source", provider)
    .map_err(|(error, _provider)| error)?;
let kernel = runtime.connect(&[(source, Permissions::READ)])?;
// Assembly drives Runtime; callers use kernel through Interface/ABI adapters.
```

`execution.rs` drives requests; `history.rs` privately owns atomic state
installation. Candidates are validated before installation. The commit step
performs no callbacks, waiting or fallible work.

`catalog.rs` selects module factories and dependencies; `modules.rs` drives
activation and cleanup. Pending work and retained state keep providers alive.
Polling a caller's result does not execute a provider.

This crate uses `no_std + alloc`. Native loading belongs in Host adapters;
Client input and presentation belong in Clients.
