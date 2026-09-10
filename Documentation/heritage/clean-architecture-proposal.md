# Clean architecture proposal

Historical proposal by ds1sqe, dated 2026-01-11, for Epic #150 and the v0.9
architecture. This note retains the design rationale; the
[heritage index](README.md) explains how to read the full original.

## Problem and proposed split

The v0.8 core combined runtime, rendering, commands, and language support.
Its manifest embedded tree-sitter language dependencies. Large event-loop
and rendering files made feature changes affect unrelated mechanisms.

The proposal separated these responsibilities using Linux-inspired names:

```text
runner/                       assembly and orchestration
  +-- plugins/                policy and language implementations
  +-- lib/drivers/            service contracts and adapters
        +-- lib/kernel/       editing mechanisms and public API
              +-- lib/arch/   platform abstraction
```

This is an ownership sketch. The proposed dependency rules also allowed
plugins to use the kernel API directly. Drivers depended inward on kernel;
peer drivers were to communicate through events. The runner assembled all
layers. `lib/arch/` covered terminals, input, signals, and memory mapping,
with Unix and Windows backends and a future WASM direction.

| Decision | Reason recorded in the proposal |
| --- | --- |
| Retire `lib/core/` | Avoid a permanent compatibility facade over mixed concerns. |
| Separate driver crates | Permit independent compilation and versioning. |
| Accept nightly Rust | Explore allocator and trait features. |
| Load modules dynamically | Support runtime extensibility and development reload. |
| Expose a versioned kernel API | Give drivers/modules an explicit dependency boundary. |
| Keep format handling outside kernel | Serialize snapshots and integrate parsers in outer layers. |

## Kernel mechanisms

| Subsystem | Proposed responsibility |
| --- | --- |
| `mm/` | Text buffers, storage, caches, and allocation helpers. |
| `ipc/` | Typed events, subscriptions, channels, and event completion scopes. |
| `core/` | Motions, text objects, registers, marks, search, and indentation. |
| `block/` | Branching undo, transactions, history, and snapshots. |
| `sched/` | Runtime lifecycle, tasks, priorities, and deferred work. |
| `api/` | Versioned public exports, experimental APIs, and internal separation. |
| `printk/` | An owned `Logger` interface and `pr_*` macros. |
| `debug/`, `panic/` | Metrics, profiling, state dumps, and failure reporting. |

Transactions grouped edits; undo nodes retained before/after cursor positions.
Snapshots exposed Rust accessors while drivers handled serialization.
Event subscriptions used drop-based cleanup, and `EventScope` tracked effects
that callers needed to await. Kernel bugs were intended to fail loudly;
module failures used recoverable, layer-specific errors.

The logging proposal placed external integrations such as tracing in
`drivers/log/`, separately from architecture-layer synchronization. Its
"std only" kernel aspiration belongs to this historical design.

## Driver and language boundary

Display owned frame buffers, windows, composition, and terminal output. Input
owned device events and key handling; LSP, network, and VFS drivers supplied
language-server access, RPC/transports, and filesystem operations.

The syntax boundary was the main example of separating contract from policy:

```text
Language module (tree-sitter + grammar + queries)
    |
    | implements
    v
SyntaxDriver (parse, update, highlights, injections, folds, indentation)
    ^
    | creates instances through SyntaxDriverFactory
LanguageRegistry (language and extension lookup)
```

The syntax contract crate was to have no tree-sitter dependency. Concrete
language modules would register factories and metadata, keeping parser choice
out of the kernel and contract.

Motions and text objects operated on actual buffer content. Display mapped
buffer positions to visual positions, including virtual text:

```text
Buffer: let x = 5;
Visual: let x: i32 = 5;
             ^^^^^
             display-only type hint
```

The type hint did not become text selected by an operator or traversed as a
word motion.

## Modules and reload

`Module` described identity, version, API requirements, dependencies, init,
exit, commands, keybindings, and subscriptions. `ModuleContext` supplied
kernel services, driver/module registries, and configuration.

The proposed loader used `libloading` and `declare_module!` entry points.
It checked API compatibility and dependencies, retained libraries while their
module objects were alive, and refused unloading a required dependency.
Development reload watched module files and reloaded changed implementations.

These sketches expressed lifecycle intent. They did not establish a safe,
stable binary interface: the original entry point returned a Rust trait
object through `extern "C"`. The full loader example is retained in history,
not as a reusable implementation here.

## Editing and rendering flows

```text
Input driver -> EventBus -> command module -> motion/edit -> Buffer
Buffer change -> syntax update -> render request -> display -> terminal
```

The rendering stages were:

1. Calculate layout and visible window bounds.
2. Read visible buffer content.
3. Obtain cached syntax spans.
4. Apply diagnostics, search, selection, cursor, and virtual text.
5. Composite base content and overlays in z-order.
6. Diff against the previous frame, flush changed cells, and swap buffers.

See [legacy lessons](legacy-memorial.md) for the failures behind the rendering,
event-completion, and stale-cache choices.

## Transition and verification intent

The original order was platform abstraction, kernel mechanisms, driver
contracts, module loading, then feature migration and retirement of old code.
Its temporary re-export schedule conflicted with the stated clean-break goal.
The later [Phase 5 strategy](phase-5-extraction.md) settled on studying legacy
behavior and implementing it afresh.
The [phase history](project-kernel-phases.md)
records the subsequent milestones; its numbering differs from this proposal.

Planned verification combined per-layer tests, mocked drivers, composed
editing tests, and benchmarks for motions, text storage, event dispatch,
rendering, and memory. Numeric budgets and claimed timings in the original
lacked a reproducible workload/environment record in this document. They are
omitted from this digest; retained performance reports live in
[perf](../../perf/README.md).

## Limits of this design

The separation of parser implementations, feature policy, and display work
was useful. The proposed kernel still assumed text buffers, line/column
positions, and motion engines. Its `BufferManager` exposed locks, file paths,
and open/save operations despite the no-I/O goal. Loader examples also placed
hosted dependencies beneath a claimed pure boundary.

These inconsistencies explain why this proposal is a historical design step.
Current [architecture](../Design/05-runtime.md) defines the domain-neutral owners,
Host contracts, and public state boundary.
