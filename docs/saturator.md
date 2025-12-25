# Saturator Architecture

This document describes the per-buffer saturator pattern used for non-blocking syntax highlighting and decoration computation.

## Problem

Synchronous syntax highlighting caused significant lag during scrolling, especially for complex grammars like Markdown:

```
Scroll input → update_visible_highlights() → ~46ms block → Render
                        ↓
             Perceived as "scroll lag"
```

The `syntax.highlight_range()` call blocked the render loop, causing dropped frames and stuttering.

## Solution: Background Saturator

The saturator is a background tokio task that:
- **Owns** syntax and decoration providers (moved from Buffer)
- **Holds** Arc references to caches (shared with Buffer/Render)
- **Computes** highlights/decorations without blocking render
- **Stores** results via lock-free `ArcSwap`
- **Signals** re-render when cache is updated

### Architecture Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                         Buffer                                │
│  ┌─────────────────┐  ┌─────────────────────────────────────┐ │
│  │ Arc<HighlightCache>│ Arc<DecorationCache>                │ │
│  │      ▲          │       ▲                                │ │
│  └──────┼──────────┴───────┼────────────────────────────────┘ │
│         │                  │                                  │
│         │ (shared via Arc) │                                  │
│         │                  │                                  │
│  ┌──────┴──────────────────┴────────────────────────────────┐ │
│  │              SaturatorHandle                              │ │
│  │  ┌────────────────────────────────────────┐               │ │
│  │  │ tx: mpsc::Sender<SaturatorRequest>     │               │ │
│  │  └────────────────────────────────────────┘               │ │
│  └───────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
         │
         │ SaturatorRequest (non-blocking try_send)
         ▼
┌──────────────────────────────────────────────────────────────┐
│               Saturator (tokio::spawn task)                   │
│                                                               │
│  Owns:                                                        │
│  ┌─────────────────────┐  ┌──────────────────────────────┐   │
│  │ syntax: Option<     │  │ decoration: Option<          │   │
│  │   Box<dyn           │  │   Box<dyn                    │   │
│  │   SyntaxProvider>>  │  │   DecorationProvider>>       │   │
│  └─────────────────────┘  └──────────────────────────────┘   │
│                                                               │
│  Shares (via Arc):                                            │
│  ┌─────────────────────┐  ┌──────────────────────────────┐   │
│  │ Arc<HighlightCache> │  │ Arc<DecorationCache>         │   │
│  └─────────────────────┘  └──────────────────────────────┘   │
│                                                               │
│  event_tx ───► InnerEvent::RenderSignal                       │
└──────────────────────────────────────────────────────────────┘
```

## Key Components

### `SaturatorRequest`

Request sent from Buffer to Saturator:

```rust
pub struct SaturatorRequest {
    pub content: String,           // Buffer content snapshot
    pub line_count: usize,         // Number of lines
    pub line_hashes: Vec<u64>,     // Per-line hashes for cache validation
    pub viewport_start: u16,       // Visible range start
    pub viewport_end: u16,         // Visible range end
}
```

### `SaturatorHandle`

Handle stored in Buffer to communicate with saturator:

```rust
pub struct SaturatorHandle {
    pub tx: mpsc::Sender<SaturatorRequest>,
}

impl SaturatorHandle {
    pub fn request_update(&self, request: SaturatorRequest) {
        // Non-blocking: drops request if saturator is busy
        let _ = self.tx.try_send(request);
    }
}
```

### Cache Design with `ArcSwap`

Both `HighlightCache` and `DecorationCache` use `ArcSwap` for lock-free read/write:

```rust
use arc_swap::ArcSwap;

pub struct HighlightCache {
    current: ArcSwap<CacheBuffer>,
}

impl HighlightCache {
    // Render reads - lock-free (~6µs)
    pub fn get_ready_highlights(&self, line_count: usize) -> Vec<Vec<LineHighlight>> {
        let buffer = self.current.load();
        // ...read from buffer...
    }

    // Saturator writes - atomic swap
    pub fn store(&self, entries: HashMap<usize, (u64, Vec<LineHighlight>)>) {
        let buffer = CacheBuffer { entries };
        self.current.store(Arc::new(buffer));
    }

    // Cache validation
    pub fn has(&self, line: usize, hash: u64) -> bool { ... }

    // For preserving entries outside viewport
    pub fn clone_entries(&self) -> HashMap<usize, (u64, Vec<LineHighlight>)> { ... }
}
```

## Data Flow

### Render Path (Fast - ~0.5ms)

```
1. Viewport scroll triggers render
2. RenderData::from_buffer() reads from cache:
   - buffer.highlight_cache.get_ready_highlights()  // ~6µs
   - buffer.decoration_cache.get_decorations()      // ~6µs
3. Render completes without blocking
```

### Saturator Path (Background - ~46ms)

```
1. Viewport changes
2. Buffer sends SaturatorRequest (non-blocking try_send)
3. Saturator receives request:
   - Check cache for misses (hash comparison)
   - If miss:
     a. Parse content via syntax.parse()
     b. Saturate injections via syntax.saturate_injections()  ← NEW
     c. Compute highlights via syntax.highlight_range()
     d. Store results via cache.store() (atomic swap)
   - Send RenderSignal
4. Runtime receives RenderSignal → triggers re-render
5. Next render picks up new cache data
```

### Language Injection Support

The saturator supports **language injections** - embedded languages within a host language:
- Markdown fenced code blocks (` ```rust `) → inject the specified language
- Rust doc comments (`///`, `//!`) → inject markdown

**Key method**: `syntax.saturate_injections(content)` eagerly pre-computes injection regions and creates language layers before `highlight_range()` is called. This ensures embedded code gets proper syntax highlighting.

**Style merging**: When rendering, if a position has both a Background decoration (e.g., code block grey tint) and a syntax highlight (e.g., keyword color), they are **merged** using `Style::merge()`:
- Decoration provides: background color
- Syntax highlight provides: foreground color
- Result: both colors are visible

## Cache Validation

Each cache entry stores `(hash, data)`:
- `hash`: Content hash of the line
- `data`: Highlights or decorations for that line

Before computing, saturator checks: `cache.has(line_idx, current_hash)`:
- **Hit**: Skip computation (content unchanged)
- **Miss**: Compute and store new data

This prevents redundant computation when lines haven't changed.

## Channel Design

The saturator uses `mpsc::channel(1)`:
- Buffer of 1 means only the latest request is queued
- `try_send()` drops old request if channel is full
- This is intentional: when scrolling fast, we only care about the latest viewport

## Performance Comparison

| Metric | Before (Sync) | After (Saturator) |
|--------|---------------|-------------------|
| Cache read | ~46ms (blocking) | ~6µs (lock-free) |
| Total render | ~50ms+ | ~0.5ms |
| Scroll feel | Laggy/stuttering | Smooth |
| First paint | Correct | May flash (cache cold) |

## Trade-offs

1. **Cold cache flash**: First render after scroll may not have highlights (shows plain text briefly)
2. **Memory**: Double buffering uses more memory than in-place mutation
3. **Complexity**: Async pattern is more complex than synchronous code

## Files

| File | Purpose |
|------|---------|
| `lib/core/src/buffer/saturator.rs` | Saturator spawn and logic |
| `lib/core/src/render/highlight_cache.rs` | Lock-free highlight cache |
| `lib/core/src/render/decoration_cache.rs` | Lock-free decoration cache |
| `lib/core/src/buffer/mod.rs` | Buffer integration (start_saturator, request_update) |
| `lib/core/src/runtime/core.rs` | Runtime integration (spawn on file open) |
| `lib/core/src/syntax/mod.rs` | `SyntaxProvider` trait with `saturate_injections()` |
| `lib/core/src/screen/mod.rs` | Style merging for decoration + highlight |
| `plugins/features/treesitter/src/injection.rs` | Injection region detection and layer management |

## Usage

### Starting a Saturator

```rust
// In Buffer
pub fn start_saturator(&mut self, event_tx: mpsc::Sender<InnerEvent>) {
    let syntax = self.syntax.take();
    let decoration = self.decoration_provider.take();

    let handle = saturator::spawn_saturator(
        syntax,
        decoration,
        Arc::clone(&self.highlight_cache),
        Arc::clone(&self.decoration_cache),
        event_tx,
    );

    self.saturator = Some(handle);
}
```

### Requesting Updates

```rust
// In Runtime, after viewport change
if buffer.has_saturator() {
    buffer.request_saturator_update(viewport_start, viewport_end);
} else {
    // Fallback to synchronous update (no syntax provider)
    buffer.update_highlights(viewport_start, viewport_end);
}
```

## Related

- [Architecture Overview](./architecture.md) - Core editor architecture
- [Syntax System](./architecture.md#syntax-system-libcoresrcsyntax) - Buffer-centric syntax design
- [Background Saturator](./architecture.md#background-saturator-performance-optimization) - Summary in architecture doc
