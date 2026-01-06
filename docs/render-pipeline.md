# Render Pipeline

This document describes reovim's render pipeline, which transforms buffer content into terminal output through a series of composable stages.

## Pipeline Overview

```
Buffer → Visibility → Highlighting → Decorations → Visual → Indent → FrameBuffer → Terminal
```

Each stage adds information to the `RenderData` structure, which accumulates all the data needed to render a window.

## RenderData Structure

The `RenderData` struct holds all rendering information for a window:

```rust
pub struct RenderData {
    // Core data
    pub lines: Vec<RenderedLine>,       // Text content
    pub cursor: CursorPosition,          // Cursor location

    // Per-line metadata
    pub visibility: Vec<LineVisibility>, // Folding state
    pub highlights: Vec<LineHighlights>, // Syntax highlighting
    pub decorations: Vec<LineDecorations>, // Language decorations
    pub signs: Vec<LineSign>,            // Sign column markers
    pub virtual_text: Vec<VirtualTextEntry>, // Inline diagnostics

    // Visual state
    pub selection: Option<SelectionRange>, // Visual mode selection
    pub indent_guides: Vec<IndentGuide>,   // Indent visualization
}
```

## Render Stages

### 1. Buffer Stage

Extracts visible lines from the buffer based on window viewport.

```rust
// Input: Buffer, viewport anchor
// Output: Raw line content

let visible_lines = buffer.lines[anchor..anchor + height];
```

### 2. Visibility Stage

Applies folding to determine which lines are visible.

```rust
pub enum LineVisibility {
    Visible,
    Collapsed { marker: String },
    Hidden,
}
```

Collapsed regions show a marker (e.g., `+-- 10 lines`). Hidden lines are skipped entirely.

### 3. Highlighting Stage

Applies syntax highlighting from treesitter.

```rust
pub struct LineHighlights {
    pub spans: Vec<HighlightSpan>,
}

pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}
```

The `HighlightCache` stores computed highlights per buffer to avoid re-parsing.

### 4. Decoration Stage

Applies language-specific decorations (markdown headings, list markers, etc.).

```rust
pub struct LineDecorations {
    pub replacements: Vec<DecorationReplacement>,
    pub conceals: Vec<ConceaRange>,
}
```

Decorations can:
- Replace text (e.g., `#` → ` `)
- Conceal characters (hide syntax markers)
- Add virtual text

### 5. Visual Stage

Applies visual mode selection highlighting.

```rust
pub struct SelectionRange {
    pub start: Position,
    pub end: Position,
    pub style: Style,
}
```

### 6. Indent Stage

Computes indent guides for visual alignment.

```rust
pub struct IndentGuide {
    pub column: usize,
    pub is_active: bool,  // Cursor's indent level
}
```

### 7. FrameBuffer Stage

Renders all data to a 2D cell grid.

```rust
pub struct FrameBuffer {
    pub cells: Vec<Vec<Cell>>,
    pub width: usize,
    pub height: usize,
}

pub struct Cell {
    pub char: char,
    pub style: Style,
}
```

### 8. Terminal Output

Diffs against previous frame and emits minimal terminal escape sequences.

```rust
// Only changed cells are written
for (row, col) in changed_cells {
    terminal.move_to(row, col);
    terminal.write_styled(cell.char, cell.style);
}
```

## Render Stage Trait

Stages implement the `RenderStage` trait:

```rust
pub trait RenderStage: Send + Sync {
    fn name(&self) -> &'static str;
    fn priority(&self) -> u32;
    fn execute(&self, data: &mut RenderData, ctx: &RenderContext);
}
```

Priority determines execution order (lower = earlier).

## Stage Registry

Stages are registered and executed via `RenderStageRegistry`:

```rust
pub struct RenderStageRegistry {
    stages: Vec<Box<dyn RenderStage>>,
}

impl RenderStageRegistry {
    pub fn register(&mut self, stage: Box<dyn RenderStage>);
    pub fn execute_all(&self, data: &mut RenderData, ctx: &RenderContext);
}
```

## Caching

### Highlight Cache

Stores syntax highlights per buffer to avoid re-parsing on every render.

```rust
pub struct HighlightCache {
    cache: HashMap<BufferId, CachedHighlights>,
}
```

Cache is invalidated when:
- Buffer content changes
- File type changes
- Treesitter grammar updates

### Decoration Cache

Stores language decorations with similar invalidation rules.

```rust
pub struct DecorationCache {
    cache: HashMap<BufferId, CachedDecorations>,
}
```

## Sign Column Integration

Signs are rendered in a dedicated column:

```rust
pub struct LineSign {
    pub char: char,       // Sign character (e.g., '●')
    pub style: Style,     // Severity-based color
    pub priority: u32,    // For conflict resolution
}
```

Sign column width is determined by `SignColumnMode`:
- `Yes`: Always 2 characters
- `Auto`: 2 characters when signs present, 0 otherwise
- `No`: Always 0
- `Number`: Signs merge into line numbers

## Virtual Text Integration

Virtual text is appended after line content:

```rust
pub struct VirtualTextEntry {
    pub line: usize,
    pub text: String,
    pub style: Style,
    pub priority: u32,
}
```

Rendering handles:
- Priority-based resolution (highest priority wins)
- Truncation with ellipsis
- Viewport clipping

## Performance Considerations

### Batch Operations

Stages process entire windows at once, not line-by-line:

```rust
// Good: Process all lines together
fn execute(&self, data: &mut RenderData, ctx: &RenderContext) {
    for line in &mut data.lines {
        self.process_line(line);
    }
}
```

### Dirty Region Tracking

Future optimization: Track which lines changed to minimize work.

```rust
pub struct DirtyRegion {
    pub start_line: usize,
    pub end_line: usize,
}
```

### Parallel Processing

CPU-bound stages (highlighting) could be parallelized using rayon:

```rust
data.lines.par_iter_mut().for_each(|line| {
    compute_highlights(line);
});
```

## Source Files

- `lib/core/src/render/mod.rs` - RenderData and core types
- `lib/core/src/render/stage.rs` - RenderStage trait
- `lib/core/src/render/registry.rs` - Stage registration
- `lib/core/src/frame/` - FrameBuffer implementation
- `lib/core/src/screen/window.rs` - Window rendering

## Adding Custom Stages

Plugins can register custom render stages:

```rust
impl Plugin for MyPlugin {
    fn register(&self, registry: &mut RenderStageRegistry) {
        registry.register(Box::new(MyCustomStage));
    }
}

struct MyCustomStage;

impl RenderStage for MyCustomStage {
    fn name(&self) -> &'static str { "my_stage" }
    fn priority(&self) -> u32 { 150 }  // After highlighting (100)

    fn execute(&self, data: &mut RenderData, ctx: &RenderContext) {
        // Add custom rendering logic
    }
}
```
