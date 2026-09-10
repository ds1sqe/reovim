# 00 — An editing journey

Reovim lets several Clients edit one working document through public APIs.
A Client handles input and rendering. The Kernel runs requests and retains
editing state; its Runtime executes work and its Core installs changes.

A **Resource** is an addressable subject. A **Buffer** holds working state
and **History**, the tree of accepted states. A **Representation** supplies
semantic data and edits for that state, such as Markdown headings.

## Edit, Undo and save

Suppose `notes.md` contains `# Draft`. This example's Markdown command renames
its only heading. Composition supplies caller access and selected services.
Optional typed helpers encode requests over the common Resource API.
`.await?` receives results or propagates errors; caller access retains recovery
for interrupted work. Runtime keeps executing independently of that wait.

```rust
// Proposed wrappers; tree, buffers, history, markdown and files are inputs.
// Each helper uses caller access; root is the granted namespace root.
let source = tree.resolve_at(root, "files/notes.md").await?;
let draft = buffers.open(source).await?; // Independent state + History.
let representation = markdown.attach(draft.state.buffer).await?;
let view = markdown.observe(representation).await?;

let changed = markdown.rename_heading(&view.snapshot, "Plan").await?;
// Buffer: "# Plan". File: "# Draft". The edit uses view's exact observed state.
let restored = history.undo(draft.history, &changed.after).await?;
// Buffer: "# Draft" again, with fresh currentness.
let redone = history.redo(draft.history, &restored.after, None).await?;
let saved = files.save(&redone.after).await?;
// Applied means the file now contains "# Plan"; later edits remain unsaved.
```

The Client turns keys or other input into the semantic command. It renders
the resulting observation using its own layout and display services.

```mermaid
flowchart LR
    Command[Client command] --> Runtime[Runtime prepares edit]
    Runtime --> Core[Core installs Buffer + History]
    Core --> Result[Runtime reports result and observation]
    Result --> View[Client renders its view]
```

Core rechecks that the edit still applies before installing state and History
together. A stale edit changes neither. Save is a separate external effect;
an unknown save outcome needs recovery, not an automatic second write.

## Another Client

```rust
// Proposed wrappers using the GUI's separately granted caller access.
// Composition exposes the same Buffer under this name.
let reference = gui_tree.resolve_at(gui_root, "buffers/draft").await?;
let joined = gui_buffers.join(reference).await?; // Same Buffer and History.
let preview = preview_api.attach(joined.state.buffer).await?;
let observed = preview_api.observe(preview).await?;
// GUI renders preview data. TUI keeps its own cursor, layout and input state.
```

A **Session** retains a Client's virtual editing space, such as its selected
Buffer. Sharing a Session is explicit; sharing a Buffer does not require it.
A **module** adds behavior through public contracts. Kernel modules supply
services/providers; Client modules supply input and presentation behavior.
Each has its own execution owner and loader.

**Composition** selects implementations and supplies granted service access.
Consumers use public Resource contracts, with optional component helpers.
They construct no private Runtime objects and can use different providers.

## Compatible implementations

```text
unofficial Client <-> official Kernel/server
official Client   <-> unofficial Kernel/server
C TUI             <-> compatible C Kernel/server
same Kernel module -> compatible Rust or C module host
// Public behavior is shared across distributions and implementation languages.
```

Compatibility covers schemas, grants, currentness, operation outcomes,
cancellation, event gaps and lifecycle. Required support is negotiated before
calls; optional differences have explicit behavior. Matching signatures alone
does not establish compatibility.

Remote calls use a versioned service protocol. Native modules use an ABI,
the binary calling and data contract, for a declared platform/architecture.
Reusing one native artifact also requires compatible imports and that ABI.
Client extensions follow their Client's separate extension contract.
Exact binary and service profiles are described in
[native and service boundaries](06.4-native-modules.md).

These chapters describe the target design. Rust blocks show declarations and
usage; comments mark proposed SDK methods. Text blocks show trees and diagrams.
Source paths name intended homes. [Product goals](../README.md) own release
scope and API stability.
