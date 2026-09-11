# Editing module

Activation publishes `models/heading` and `models/keys`. Both use the existing
Buffer/History authority; the module imports Interface and provider APIs.
Retained Buffers keep the module loaded until explicitly closed.

Caller helpers follow the usage in
[07-examples.md](../../Documentation/Design/07-examples.md):

```rust
// Inside an async caller with granted kernel access.
use reovim_module_editing as editing;
use reovim_kernel_interface::buffer::{self, ClosePolicy};

let model = kernel.resolve("models/heading").await?;
let opened = editing::open(&kernel, model, b"# Notes").await?;
let changed = editing::replace(&kernel, &opened, b"Draft").await?;
buffer::close(&kernel, changed.state, ClosePolicy::DiscardLocal).await?;
```

These helpers encode component schemas over ordinary Resource calls. Unknown
outcomes retain request recovery; callers must not blindly repeat the edit.
Public Rustdoc examples compile with the module.

The native `heading` example exports the same heading operations with
`rv_module!`'s synchronous Model declaration, using the direct C Model ABI.
Build it with `cargo build --manifest-path product/Cargo.toml
-p reovim-module-editing --example heading`. It declares `heading.model` schema
1 and publishes at the configured mount. Its immutable states keep native code
loaded through Buffer/History retention; no async executor or raw pointers are
needed in the Rust provider.

The Resource shell can call the same contracts directly. With a private boot
catalog labeled `editing`, activate with
`write catalog/editing kernel.module.activate 1`. Model creation
returns Buffer and History references; edits carry the Buffer reference and
observed revision. `scripts/e2e.py` exercises edit, stale rejection, Undo,
source fidelity and unload blocked by retained state.
