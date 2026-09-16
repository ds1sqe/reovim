# Design

What the Kernel and the wire enforce. Anything a Driver, Module or Plugin
may choose otherwise is not here; it belongs to a package or to that
Module's own doc.

- [Core](00-core.md): the tree, Modules, Drivers, the vtable,
  declarations, rights. What exists.
- [Core: runtime](01-core-runtime.md): Buffer, Reply, Execution, Watch,
  Log, Pressure. What happens.
- [Client](02-client.md): what the wire enforces.
- [Walkthroughs](03-walkthroughs.md): text, LSP, mesh against the above.
- [References](references.md): external systems that shaped it.
- [Plugins](plugins/future.md): Client-side research, interfaces still open.

Packages such as `history`, `text`, `publish` are declarations outside the
Kernel. Each gets one page beside its code when that code exists.
