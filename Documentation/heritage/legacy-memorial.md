# Legacy lessons

The v0.1.0-v0.8.x codebase supplied behavior and failure lessons for Epic #150.
It was archived in Phase 5 (#215) in January 2026, then removed from the tree
in v0.14.3. The [historical source][source] remains accessible.

[source]: https://github.com/ds1sqe/reovim/tree/81806439/archive/pre_kernel

## What the legacy code contributed

| Area | Useful experience |
| --- | --- |
| `lib/core/runtime/`, `event_bus/` | Scheduling, event dispatch, and effect completion. |
| `lib/core/buffer/`, `command/` | Rope-based text editing, commands, and undo integration. |
| `lib/core/screen/`, `frame/`, `compositor/`, `render/` | Damage tracking, windows, composition, and style merging. |
| `lib/core/plugin/`, `option/`, `keystroke/` | Lifecycle, typed options, and key sequences. |
| `lib/core/highlight/`, `lib/lsp/` | Incremental syntax and asynchronous language services. |
| `lib/sys/`, `runner/` | System adaptation and application assembly. |
| `tools/reo-cli/` | Instance discovery, key injection, state queries, and scripting. |
| `tools/bench/` | Benchmark workloads and measurement utilities. |

The standalone CLI became `reovim cli` in Phase 6. The
[extraction map](phase-5-extraction.md) records where core concepts went.

Feature plugins exercised the boundaries through concrete consumers:

- `microscope`, `pickers`, `explorer`, and `range-finder`: asynchronous search,
  previews, lazy trees, reusable selection UI, and range selection.
- `treesitter`, `lsp`, `completion`, and `cmdline-completion`: parsing, RPC,
  capability negotiation, asynchronous sources, and command completion.
- `statusline`, `which-key`, `pair`, and `notification`: composable status,
  keymap introspection, insert hooks, timeouts, and queued feedback.
- `context` and `sticky-context`: AST queries and scroll synchronization.
- `health-check`, `settings-menu`, `profiles`, and `landing`: diagnostics,
  settings forms, persistent configuration, and startup UI.

Language plugins added environment-specific lessons: Cargo/rust-analyzer,
Python virtual environments, JavaScript module resolution, C/C++ compile
commands, shell detection, JSON schemas, Markdown decorations, and TOML syntax.

## Terminal flicker

Direct terminal writes flickered. The v0.6.0 frame-buffer experiments removed
flicker but added overhead; v0.6.1 hid the cursor during rendering. v0.6.6
combined front/back buffers with a cell diff.

```text
Render next frame -> diff front/back -> write changed cells -> swap buffers
```

The swap avoided copying a frame. Hiding/showing the cursor around output
prevented cursor artifacts; resetting style before setting new attributes
prevented underline and bold from bleeding into later cells.

The lasting lesson is to separate producing a frame from presenting changes.
The original implementation lived under `lib/core/frame/` and informed the
display driver.

## Auto-pair latency (#85, #86, #93-#97)

Typing an opening brace could delay its closing pair under parallel load.
The recorded investigation found several bottlenecks rather than one:

| Change | Bottleneck addressed |
| --- | --- |
| #85: dedicated event-bus thread | Event processing starved on the Tokio runtime. |
| #93: lazy tree-sitter queries | Eager query compilation blocked startup. |
| #94: background pre-compilation | First-use compilation delayed interaction. |
| #95: priority channels | Lower-priority work delayed input. |
| #96: ArcSwap handler dispatch | Read-lock contention delayed event delivery. |

Fixing one layer exposed the next. The original note reported latency falling
from about 300 ms to below 10 ms across these changes, but supplied no
reproducible workload/environment record. Treat those figures as historical
reports, not a current baseline; retained reports are in
[perf](../../perf/README.md).

## Effect completion and progressive display (#70, #98, #99)

Auto-pair tests sometimes observed `{` before the handler inserted `}`.
The assertion raced the effects of the injected key.

`EventScope` counted in-flight events and notified waiters when they drained.
An RPC/test path could wait with a timeout before querying state. Interactive
paths favored immediate feedback and allowed later presentation updates;
tests needing completed effects required explicit synchronization.

```text
Key injection -> handler -> derived effects -> scope complete -> assertion
```

The same progressive approach displayed raw text first, then syntax and
Markdown decorations (#89, #93, #94). Context queries (#130) also arrived
asynchronously. Delayed derived display was a deliberate latency tradeoff;
it did not itself prove effect completion or cache consistency.

## Competing cache writers (#115)

The decoration saturator had a fast viewport path and a slow full-file path.
Both wrote the same cache. A full-file result could finish after a newer
viewport result and overwrite it with an older snapshot's decorations.

```text
Viewport path:   compute old -> edit -> compute current -> cache current
Full-file path:  compute old ---------------------------> cache old
Render:          request current content -> reject old entry on lookup
```

The recorded fix stored each line's `content_hash` with its decorations and
returned a cached value only when that hash matched the requested content.
The representative lookup was:

```rust
self.entries.get(&line_idx)
    .filter(|entry| entry.content_hash == current_hash)
    .map(|entry| entry.decorations.clone())
```

This rejected mismatched content at read time. It did **not** prevent an old
writer from replacing a newer entry; that could still turn a valid result
into a cache miss. Nor did a line hash identify all surrounding parse context.
The original "fast path wins" wording overstated the guarantee. The useful
lesson is to validate asynchronous results against all inputs that determine
their meaning and coordinate competing writers.

Historical source: `lib/core/render/decoration_cache.rs`.

## Ownership and extension patterns

| Pattern | Problem and lesson |
| --- | --- |
| Temporary state extraction | `mem::take` let runtime code process state while calling back into its owner, then restore it. Restoration is part of the operation. |
| Command registry (#13) | Hardcoded `Settings`/`Health` cases coupled core to plugins. Plugins registered named handlers; core supplied dispatch. |
| Plugin lifecycle | Load, init, ready, active/suspended, shutdown, and unload made ownership transitions explicit. |
| Compositor | Separate background, content, decorations, overlays, and cursor layers made presentation order explicit. |
| EventBus | Publishers and subscribers communicated through typed events instead of direct feature dependencies. |

The largest legacy objects combined too many responsibilities: `CorePlugin`
owned many features, `Runtime` owned nearly all state, and `Screen` mixed
rendering with layout. The replacement separated mechanisms, service
contracts, and policy. These lessons remain useful; their historical API and
owner choices are recorded in the
[architecture proposal](clean-architecture-proposal.md).
