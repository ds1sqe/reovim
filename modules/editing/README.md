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

The Resource shell can call the same contracts directly. Activate with
`write modules/catalog/editing kernel.module.activate 1`. Model creation
returns Buffer and History references; edits carry the Buffer reference and
observed revision. `scripts/e2e.py` exercises edit, stale rejection, Undo,
source fidelity and unload blocked by retained state.
