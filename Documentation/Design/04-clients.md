# 04 — Clients, context and input

Clients own all user input handling, user views, rendering and user-facing
output. Kernel owns none of those behaviors. Kernel APIs exchange semantic
requests/results and Resource observations; Host/Resource I/O is a separate
contract from presenting an editing interface to a user.

| Role | Owner and purpose |
| --- | --- |
| Client | Running participant; its Runtime owns input and presentation |
| Connection | Granted caller access to one Runtime, local or remote |
| Session | Client context retained as a Resource at the Kernel/server |

Client code homes: `clients/<family>/{core,contract,runtime}`; Session
ownership: `kernel/runtime`. Roles need no empty packages.
[Runtime](05-runtime.md) owns execution; [release scope](../README.md) owns
family/language requirements.

These code homes describe the official Clients.
An independent Client can use another language, internal architecture or
extension system. Interoperation follows the public
[compatibility contract](00-overview.md).
Optional [Client Resources](07.3-client-resources.md) expose granted state or
services without making Kernel own input or presentation.

## Session participation

[Session API](04.1-session.md) owns fields, calls and retention. Its agreed
schema defines context meaning, sharing and retention under Client policy.
Server storage grants Kernel no input/view/rendering responsibility or
automatic referenced-data access. A Session is the retained virtual editing
space; each Client uses its own by default. Attaching another Client to it or
sharing selected module state is explicit. One lost participant cannot stop
others. [Kernel modules](06-modules.md) may have Session/global state; Client
extensions can retain their own separately owned Session Resources.

```text
attach Session -> open/join Buffer -> attach Representation -> observe
// Attach returns participation; read/follow obtains context.
// A granted headless caller may edit without a Session.

switch active Buffer -> update Session context
// Preserve unsaved Buffers. This changes neither content nor History.

seek retained context -> select this participation's historical view
follow                -> obtain an authorized current checkpoint + watch
// Seek does not change live context, Undo a Buffer or replay effects.
// Missing/unsupported history is unavailable, never a latest-state fallback.

detach -> end participation; Session survives under its retention owner
close  -> release declared references; unrelated shared Buffers survive
// Retention is bounded and does not promise disk/server-crash persistence.
```

## Client modules

Within one official client family, Client Core enforces local contracts and
dispatches; Client modules supply policy through Contract;
Runtime executes and owns lifetime. This Client module system has no implicit
relationship to Kernel modprobe or its module ABI. It creates no second
Buffer authority or cross-client synchronization.

Client modules can logically depend on Kernel services and on other Client
modules. [Composition](05.1-composition.md) supplies that granted public access;
each Runtime retains its own lifecycle and grants.

Clients own mode, focus, cursors, selections, scrolling, layout, hit-testing
and drawing. Family contracts expose applicable
input/commands/views/decorations/ output; a CLI can produce bounded output
without a viewport. Extensions use public contracts, never another family's
internals.

Vim/Emacs supply essential first-party editing behavior through Client contracts
and public Kernel/Domain APIs. Their loaded instances execute in Client
Runtime; their logical state may be retained in Session Resources. They
neither introduce another Buffer/History authority nor put input resolution
inside Kernel. [Composition](05.1-composition.md) supplies their access.

```text
Vim:   modes, counts, operators, motions, registers and key bindings
Emacs: prefix commands, mark/selection, kill-ring behavior and key bindings
// A companion Kernel semantic service uses a separate module contract.
// Upstream plugin/runtime compatibility is not implied by these behaviors.
```

The [semantic module example](06.3-module-composition.md) shows Vim/Emacs using
generic navigation Resources and a vim-motion-helper that reads semantic
items, observes pending input and renders only this Client's help surface.
Shared Session membership does not share pending commands or help visibility.

Accepted edit/Undo/Redo reports state. Each client may follow/keep/restore its
context. Old selections stay stale until revalidated; a fresh edit need not
use them. Correlate acceptance with matching observation before
editing-dependent responses, regardless of arrival order. Failed preview
cannot undo acceptance. [Semantic values](03.1-representation.md) own
Position/Selection/translation.

## Keymaps and resolver

Client Core normalizes input and runs resolver mechanics. Client extension
policy maps active context and input to commands. Device adapters supply no
edit meaning.

```rust
// Proposed Client APIs; keymaps and resolver are granted Client APIs.
keymaps.register(&owner, sequence, predicates, command, priority)?;
// Validate availability, grants and bounds before publishing.
// Identical selectors/priority conflict unless owner explicitly replaces.
let resolved = resolver.resolve(&input, &active_context)?;
// Specific active scope before family default; higher priority within scope.
// Equal active matches: ambiguous, execute neither.
keymaps.unregister(&owner)?; // Remove contributions; fence their pending input.
```

Registration order never ranks a command.

| Chord state | Resolver action |
| --- | --- |
| Complete and also a prefix | Wait to monotonic deadline, then run complete match |
| Prefix only at deadline | Clear pending chord |
| Mismatch | Clear prefix; resolve new input once |
| Context change, removal or input loss | Fence work from the prior context |

Chord length and wait are bounded. Logical keys differ from committed
text/composition; never insert the same character twice. Families define
paste, pointer and composition handling. Map pointers through current
correspondence; stale/ambiguous mapping rejects. Raw keys/screen coordinates
never enter Kernel editing state. Input recovery cannot replay an uncertain
edit.

## Presentation

View layout, drawing and delivery to the user are Client responsibilities,
including terminal output, GUI frames and SSR. Hosting a renderer in a server
process does not make it part of Kernel. A semantic Representation may supply
preview data; Client code turns that data into the user-visible view.

Presentation contracts select cells, raster, vector, video or structured
output; CPU/GPU support is separate. TUI may offer Kitty/sixel/raster without
promising video/GPU. Contracts define formats, coordinates, device lifetime,
ordering, completion, bounds, pressure and teardown. Contributions bind
current view/frame correspondence; native handles, escapes, codecs and GPU
queues stay in adapters.

Device/capability loss fences work and requires renewed readiness. Required
loss stops the path; optional loss names degradation. Fidelity-changing
fallback needs acceptance. SSR is neither editing authority nor interactive
readiness; GUI framework/deployment remain open.

## Sharing across Clients

Clients can share explicit semantic/extension context. Each scope
defines authority, schema, members, grants, ordering, conflicts and retention.
Join/follow needs an authorized checkpoint; gaps/incompatible reconnect
require recovery. Isolate slow members. Shared selection/presence has its own
schema; each client maps it to a view. Frames/pixels/GPU state are not shared
truth. Session retention or a common source does not merge Buffers or enable
context sharing.

Peer focus can be an observed Session participation Resource. The receiving
Client maps its semantic anchors and chooses whether to follow. Framebuffer
inspection is a separate optional export; neither makes Kernel render a view.
See the [future focus surface](07.3-client-resources.md).

Input-to-presentation diagnostics follow [debugging](../debugging.md);
[performance](../performance.md) measures the actual Client and transport paths.
