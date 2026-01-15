# Legacy Memorial

> *In memory of `lib/core` and `plugins/` — the foundation upon which we built.*

This document honors the legacy codebase (v0.1.0–v0.8.x) that served as the reference for Reovim's architecture overhaul. Though the code is archived, the concepts and lessons live on in the new architecture.

---

## The Legacy Architecture

```
archive/
├── lib/
│   ├── core/          # The heart of v0.8.x
│   ├── lsp/           # Language Server Protocol
│   └── sys/           # System utilities
├── plugins/
│   ├── features/      # 18 feature plugins
│   └── languages/     # 8 language plugins
├── runner/            # Original runner
└── tools/
    ├── reo-cli/       # Standalone CLI tool
    └── bench/         # Benchmarking utilities
```

---

## lib/core — The Heart

The monolithic core that did everything. We studied it, learned from it, and carefully separated its concerns.

### What We Studied

| Component | Lines | What It Taught Us |
|-----------|-------|-------------------|
| `runtime/` | ~2000 | Event loop design, async task scheduling |
| `buffer/` | ~1500 | Rope data structures, efficient text manipulation |
| `event_bus/` | ~500 | Decoupled communication between components |
| `screen/` | ~800 | Terminal abstraction, damage tracking |
| `compositor/` | ~600 | Layer-based rendering, z-ordering |
| `frame/` | ~400 | Window layout, splitting, focus management |
| `plugin/` | ~1200 | Plugin lifecycle, hot-reload mechanics |
| `render/` | ~700 | Cell-based rendering, style merging |
| `highlight/` | ~900 | Syntax highlighting integration |
| `keystroke/` | ~400 | Key parsing, sequence matching |
| `command/` | ~600 | Command pattern, undo/redo integration |
| `option/` | ~300 | Configuration system, type-safe options |

### Concepts Extracted

```
lib/core/runtime/     →  lib/kernel/sched/     (Scheduler)
lib/core/buffer/      →  lib/kernel/mm/        (Memory Management)
lib/core/event_bus/   →  lib/kernel/ipc/       (Inter-Process Communication)
lib/core/option/      →  lib/kernel/core/      (Core Types)
lib/core/screen/      →  lib/drivers/display/  (Display Driver)
lib/core/compositor/  →  lib/drivers/display/  (Compositor)
lib/core/frame/       →  lib/drivers/display/  (Window Views)
lib/core/keystroke/   →  lib/drivers/input/    (Input Driver)
lib/core/highlight/   →  lib/drivers/syntax/   (Syntax Driver)
lib/core/command/     →  lib/drivers/command/  (Command Driver)
lib/core/plugin/      →  modules/              (Module System)
```

---

## plugins/features — The Extensions

18 feature plugins that demonstrated what a text editor could be.

| Plugin | What It Did | What We Learned |
|--------|-------------|-----------------|
| `microscope` | Fuzzy finder (telescope-like) | Async search, preview architecture |
| `explorer` | File tree | Tree data structures, lazy loading |
| `lsp` | Language Server Protocol | Async RPC, capability negotiation |
| `treesitter` | Syntax highlighting | Incremental parsing, query system |
| `completion` | Autocompletion | Popup rendering, async sources |
| `statusline` | Status display | Composable segments, dynamic updates |
| `which-key` | Key binding hints | Timeout-based UI, keymap introspection |
| `pair` | Auto-pairing | Insert hooks, context awareness |
| `notification` | Toast notifications | Animation, queue management |
| `context` | Code context | Treesitter queries, sticky headers |
| `sticky-context` | Persistent context | Scroll synchronization |
| `pickers` | Selection UIs | Generic picker pattern |
| `range-finder` | Range selection | Multi-cursor foundation |
| `cmdline-completion` | Command completion | Vim command parsing |
| `health-check` | Diagnostics | System introspection |
| `settings-menu` | Settings UI | TUI form components |
| `profiles` | Config profiles | Serialization, switching |
| `landing` | Welcome screen | First-run experience |

---

## plugins/languages — The Polyglots

8 language plugins that brought IDE features to each language.

| Language | Features | Patterns Learned |
|----------|----------|------------------|
| `rust` | LSP, treesitter, formatting, debugging | Cargo integration, rust-analyzer quirks |
| `python` | LSP, treesitter, formatting | Virtual environment detection |
| `javascript` | LSP, treesitter, formatting | Node module resolution |
| `c` | LSP, treesitter | Compile commands, header handling |
| `bash` | Treesitter, shellcheck | Script detection |
| `json` | Treesitter, formatting, schema | JSON Schema validation |
| `markdown` | Treesitter, preview, concealment | Decoration rendering |
| `toml` | Treesitter | Config file patterns |

---

## tools/reo-cli — The Companion

A standalone CLI tool for interacting with Reovim instances.

| Feature | What It Did | What We Learned |
|---------|-------------|-----------------|
| Instance discovery | Find running Reovim servers | Port file conventions |
| Key injection | Send keystrokes remotely | RPC protocol design |
| State queries | Get mode, cursor, buffer info | JSON-RPC patterns |
| Scripting | Automate editor operations | CLI UX principles |

*Became:* `reovim cli` subcommand in Phase 6

---

## Patterns Discovered

Wisdom extracted from years of iteration:

### The Flickering Solution

**The Problem:** Direct terminal rendering caused visible flickering during screen updates.

**The Journey:**

| Version | Approach | Result |
|---------|----------|--------|
| Pre-v0.6.0 | Direct terminal writes | Flickering on every update |
| v0.6.0 | FrameBuffer + 3 strategies | Zero flicker, but +6x overhead |
| v0.6.1 | Hide cursor during render | Fixed cursor blinking artifact |
| v0.6.6 | Double-buffer with cell diff | Zero flicker, minimal overhead |

**The Final Solution (v0.6.6):**

```rust
// Double-buffer swap pattern
struct FrameRenderer {
    front: FrameBuffer,  // Currently displayed
    back: FrameBuffer,   // Being rendered to
}

fn flush(&mut self) {
    let commands = self.compute_diff();  // Only changed cells
    for cmd in commands { write(cmd); }
    std::mem::swap(&mut self.front, &mut self.back);  // O(1) swap
}
```

**Key Techniques:**
- **Front/back buffer**: Render to back, display from front
- **Cell-by-cell diff**: Only changed cells sent to terminal
- **O(1) swap**: `std::mem::swap()` instead of data copy
- **Cursor hiding**: Wrap render with Hide/Show cursor sequences
- **Reset-before-set**: Prevent style bleed (underline, bold)

*Discovered in:* `lib/core/frame/` → now `lib/drivers/display/frame/`

### The Auto-Pair Latency Saga (#85, #86, #93-#97)

**The Problem:** Auto-pair (typing `{` inserts `}`) had **300ms latency** under parallel load.

**The Journey:**

| Issue | Fix | Latency |
|-------|-----|---------|
| Initial | - | 300ms |
| #85 | Dedicated thread for event bus | 100ms |
| #93 | Lazy tree-sitter query compilation | 80ms |
| #94 | Background query pre-compilation | 60ms |
| #95 | Priority channels for user input | 40ms |
| #96 | ArcSwap for lock-free dispatch | <10ms |

**Root Causes Discovered:**

1. **Tokio starvation**: Event bus on tokio runtime blocked by parallel tests
2. **Query compilation**: Tree-sitter queries (~365ms total) blocked startup
3. **Lock contention**: `RwLock` in `EventBus::dispatch()` caused waits
4. **Event ordering**: Low-priority events delayed user input

**Key Solutions:**

```rust
// #85: Dedicated OS thread (not tokio)
std::thread::spawn(move || { event_processor.run(); });

// #93: Lazy compilation
fn get_highlights(&self) -> &Query {
    self.highlights.get_or_init(|| compile_query(...))
}

// #95: Priority channels
enum Priority { High, Normal, Low }
channel.send_with_priority(event, Priority::High);

// #96: Lock-free dispatch with ArcSwap
let handlers = self.handlers.load();  // No lock!
for handler in handlers.iter() { ... }
```

**Lesson:** Performance bugs often have multiple causes layered together. Fixing one reveals the next bottleneck.

*Discovered in:* `lib/core/runtime/`, `lib/core/event_bus/`, tree-sitter integration

### The Flaky Test Saga (#70, #98, #99)

**The Problem:** Tests failed intermittently - auto-pair `{` → `{}` sometimes showed only `{`.

```rust
// Test would fail randomly
result.assert_buffer_contains("{}");
// Actual: "{"  ← closing brace not inserted yet!
```

**Root Cause:** Race condition between:
1. Test assertion executing
2. Auto-pair event handler completing

**The Solution:** EventScope - GC-like reference counting for event lifecycle.

```rust
// EventScope tracks in-flight events
struct EventScope {
    in_flight: Arc<AtomicUsize>,
    completion: Arc<Notify>,
}

impl EventScope {
    fn increment(&self) { /* Event emitted */ }
    fn decrement(&self) { /* Event dispatch complete */ }
    async fn wait(&self) { /* Block until counter = 0 */ }
}

// RPC waits for all effects before responding
async fn handle_keys(&self, keys: &str) -> Response {
    let scope = EventScope::new();
    self.inject_keys(keys, scope.clone());
    scope.wait_timeout(Duration::from_secs(3)).await;  // Wait for effects!
    self.get_state()
}
```

**The Trade-off: Allow the Race**

We chose **fast feedback over strict synchronization**:

```
Option A: Wait for all effects (strict)
  └→ Guaranteed correctness, but slower response

Option B: Allow the race (chosen)
  └→ Fast feedback, occasional consistency gaps
```

**Why we allow the race:**
- User input feels instant (no waiting for side effects)
- Most effects complete before next frame anyway
- Strict waiting adds latency to every keystroke

**Known trade-off:**
- Some effects may not be visible immediately
- Tests may need explicit sync points
- Future issues possible with complex effect chains

**Key Insight:** Sometimes "correct but slow" is worse than "fast with known edge cases". Document the trade-off, don't hide it.

**The Pattern Applied Everywhere:**

| Feature | Lazy Behavior | Visual Gap |
|---------|---------------|------------|
| **Tree-sitter** (#93) | Queries compile on first use | No highlighting for ~100ms |
| **Markdown decoration** (#89) | Decorations render after parse | Raw markdown briefly visible |
| **Syntax highlighting** | Background compilation (#94) | Gradual highlight appearance |
| **Context provider** (#130) | AST queries are async | Breadcrumbs appear delayed |

```
User opens file
    │
    ├→ Instant: Buffer displayed (raw text)
    ├→ ~50ms: Basic syntax (cached queries)
    ├→ ~100ms: Full highlighting (compiled)
    └→ ~200ms: Decorations (markdown tables, etc.)
```

**Philosophy:** Show *something* immediately, enhance progressively. Users perceive "instant open" even if decorations lag.

**The Conflict: Viewport vs Full-File (#115)**

The saturator's two-path architecture created internal conflicts:

```rust
// Two channels fighting for the same cache
struct SaturatorHandle {
    viewport_tx: mpsc::Sender<Request>,  // Path 1: Immediate
    fullfile_tx: mpsc::Sender<Request>,  // Path 2: Background
}
```

**The Fight:**

```
Time 0:  User opens file
         └→ Path 1: Compute viewport (lines 1-50)
         └→ Path 2: Start full-file computation (lines 1-10000)

Time 50ms:  Path 1 completes → cache updated with lines 1-50

Time 100ms: User scrolls to line 100
            └→ Path 1: Compute viewport (lines 100-150)

Time 150ms: Path 2 still running... (at line 5000)

Time 200ms: Path 1 completes → cache updated with lines 100-150
            Path 2 still running... (at line 7000)

Time 300ms: Path 2 completes → OVERWRITES cache with stale data!
            └→ Lines 100-150 now show OLD decorations
```

**The Issues:**
- Full-file computation becomes stale mid-flight
- Viewport updates can be overwritten by slow background task
- Two writers to same cache without coordination
- `biased` select! helps but doesn't fully solve

**Lesson:** Two async paths writing to the same cache need careful coordination. The "allow the race" philosophy has limits when paths have different latencies.

**The Solution: Fast-Win via Content Hash**

The cache uses `content_hash` per line - stale writes are automatically discarded:

```rust
struct CacheEntry {
    content_hash: u64,     // Hash of line content when computed
    decorations: Vec<Decoration>,
}

// Get only returns if hash matches CURRENT content
pub fn get(&self, line_idx: usize, current_hash: u64) -> Option<Vec<Decoration>> {
    self.entries.get(&line_idx)
        .filter(|e| e.content_hash == current_hash)  // FAST-WIN CHECK
        .map(|e| e.decorations.clone())
}
```

**How it works:**

```
Time 0:    File opened, line 100 hash = 0xAAA
           └→ Viewport: compute line 100 with hash 0xAAA
           └→ Full-file: start computing (snapshot has hash 0xAAA)

Time 50ms:  Viewport done → cache[100] = { hash: 0xAAA, deco: [...] }

Time 100ms: User edits line 100 → hash changes to 0xBBB
            └→ Viewport: compute line 100 with hash 0xBBB
            Full-file still running with old snapshot...

Time 150ms: Viewport done → cache[100] = { hash: 0xBBB, deco: [...] }

Time 300ms: Full-file done → tries to write { hash: 0xAAA, deco: [...] }
            Render requests: get(100, 0xBBB)
            └→ 0xAAA != 0xBBB → CACHE MISS → stale data ignored!
```

**Key Insight:** Hash-based validation = automatic stale rejection. The fast path's results naturally "win" because they have the current hash.

*Discovered in:* `lib/core/render/decoration_cache.rs` — pattern for cache consistency

### The Double-Swap Pattern

```rust
// Problem: Can't call &mut self method while borrowing self
// Solution: Temporarily swap state out

let mut state = std::mem::take(&mut self.state);
state.process(|item| {
    // Can now mutate freely
});
self.state = state;
```

*Discovered in:* `lib/core/runtime/` event processing

### The Ex-Command Registry (#13)

**The Problem:** Core had hardcoded plugin commands, creating tight coupling.

```rust
// BEFORE: Core knew about every plugin command
pub enum ExCommand {
    Settings,  // ← Plugin-specific in core!
    Health,    // ← Plugin-specific in core!
}

// handlers.rs had plugin-specific cases
ExCommand::Settings => { /* ... */ }
ExCommand::Health => { /* ... */ }
```

**The Solution:** Generic command registration and dispatch.

```rust
// AFTER: Plugins register their own commands
impl Plugin for HealthCheckPlugin {
    fn build(&self, ctx: &mut PluginContext) {
        ctx.register_ex_command("health", |runtime, args| {
            runtime.event_bus.emit(HealthCheckOpen);
        });
    }
}

// Core handles generic dispatch
ExCommand::ExecuteCommand(cmd_name) => {
    if let Some(handler) = self.command_registry.get(&cmd_name) {
        handler.execute(self);
    }
}
```

**Key Insight:** Core should provide *mechanism* (command registry), not *policy* (specific commands).

*Discovered in:* `lib/core/runtime/handlers.rs` — led to mechanism vs policy separation

### The Plugin Lifecycle

```
Load → Init → Ready → [Active ↔ Suspended] → Shutdown → Unload
```

*Discovered in:* `lib/core/plugin/` — became the Module trait

### The Compositor Pattern

```
Background → Buffer → Decorations → Overlays → Cursor
     ↑           ↑           ↑           ↑
   z=0         z=10        z=20        z=30
```

*Discovered in:* `lib/core/compositor/` — now in `lib/drivers/display/`

### Event-Driven Decoupling

```
Component A  →  EventBus  →  Component B
    ↓              ↓              ↓
 publish()    route by type    subscribe()
```

*Discovered in:* `lib/core/event_bus/` — now in `lib/kernel/ipc/`

---

## The God Objects

What we learned NOT to do:

| Object | Peak Size | Problem | New Design |
|--------|-----------|---------|------------|
| `CorePlugin` | 969 lines | Did everything | Split into 6+ modules |
| `Runtime` | 1200 lines | Owned all state | Kernel services |
| `Screen` | 800 lines | Rendering + layout | Display driver + layout policy |

---

## Statistics

**Legacy Codebase (v0.8.x):**
- `lib/core/`: ~45 modules, ~15,000 lines
- `plugins/features/`: 18 plugins, ~8,000 lines
- `plugins/languages/`: 8 plugins, ~2,000 lines
- Total: ~25,000 lines of Rust

**What It Became (v0.9.0+):**
- `lib/kernel/`: 6 subsystems, clean API boundary
- `lib/drivers/`: 8 drivers, trait-based contracts
- `modules/`: Policy-only, mechanism-free

---

## Acknowledgment

The legacy code served Reovim from its inception. It was:

- **Ambitious** — attempting features beyond most terminal editors
- **Educational** — teaching us what works and what doesn't
- **Foundational** — providing the concepts for the new architecture

Though archived, its spirit lives on in every kernel subsystem, every driver contract, and every module that inherits from its design.

---

*Archived: Phase 5 (#215) — January 2026*

*"The code is gone, but the knowledge remains."*
