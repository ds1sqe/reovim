# Render Pipeline

This document describes the data transformation stages that convert buffer content to terminal output.

## RenderData Structure

The `RenderData` struct holds all rendering information for a window:

```rust
pub struct RenderData {
    /// Base content (lines of text)
    pub lines: Vec<String>,

    /// Per-line visibility state (for folding)
    pub visibility: Vec<LineVisibility>,

    /// Per-line highlights (syntax, search, etc.)
    pub highlights: Vec<Vec<LineHighlight>>,

    /// Per-line decorations (conceals, backgrounds)
    pub decorations: Vec<Vec<Decoration>>,

    /// Per-line signs for gutter
    pub signs: Vec<LineSign>,

    /// Per-line virtual text for end-of-line display
    pub virtual_texts: Vec<Option<VirtualTextEntry>>,

    /// Metadata
    pub buffer_id: usize,
    pub window_id: usize,
    pub window_bounds: Bounds,

    /// Cursor position (line, column) for bracket matching etc.
    pub cursor: (usize, usize),
}
```

## Pipeline Stages

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
    /// Line is visible
    Visible,
    /// Line is hidden (folded)
    Hidden,
    /// Line is a fold marker showing preview text
    FoldMarker {
        /// Preview text to display
        preview: String,
        /// Number of hidden lines
        hidden_lines: u32,
    },
}
```

`FoldMarker` variants show a preview (e.g., `fn main() {...}`). Hidden lines are skipped entirely.

### 3. Highlighting Stage

Applies syntax highlighting from treesitter.

```rust
/// Highlight span for a portion of a line
pub struct LineHighlight {
    /// Starting column (0-indexed)
    pub start_col: usize,
    /// Ending column (exclusive)
    pub end_col: usize,
    /// Style to apply
    pub style: Style,
}
```

The `HighlightCache` stores computed highlights per buffer to avoid re-parsing.

### 4. Decoration Stage

Applies language-specific decorations (markdown headings, list markers, etc.).

```rust
/// Decoration for a portion of a line
pub struct Decoration {
    /// Starting column (0-indexed)
    pub start_col: usize,
    /// Ending column (exclusive)
    pub end_col: usize,
    /// Type of decoration
    pub kind: DecorationKind,
}

/// Type of decoration
pub enum DecorationKind {
    /// Conceal text with replacement
    Conceal { replacement: Option<String> },
    /// Background highlight
    Background { style: Style },
    /// Inline virtual text
    VirtualText { text: String, style: Style },
}
```

Decorations can:
- Conceal text with optional replacement (e.g., `#` → ` `)
- Add background highlighting
- Add inline virtual text

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

## RenderStage Trait

Stages implement the `RenderStage` trait:

```rust
/// Pipeline stage that transforms render data
///
/// Each stage receives render data and returns augmented data.
/// Stages should be immutable transformations.
pub trait RenderStage: Send + Sync {
    /// Transform render data
    ///
    /// Takes input data and returns modified data for the next stage.
    fn transform(&self, input: RenderData, ctx: &RenderContext<'_>) -> RenderData;

    /// Stage name for debugging
    fn name(&self) -> &'static str;
}
```

Stages are executed in registration order. Each stage takes ownership of `RenderData`, transforms it, and returns the modified data for the next stage.

## Stage Registry

Stages are registered via `RenderStageRegistry`:

```rust
pub struct RenderStageRegistry {
    stages: Vec<Arc<dyn RenderStage>>,
}

impl RenderStageRegistry {
    /// Create a new empty registry
    pub fn new() -> Self;

    /// Register a render stage
    /// Stages are executed in registration order during rendering.
    pub fn register(&mut self, stage: Arc<dyn RenderStage>);

    /// Get all registered stages
    pub fn stages(&self) -> &[Arc<dyn RenderStage>];
}
```

Note: Stages use `Arc` (not `Box`) to allow shared ownership across async contexts.

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

## Performance

### Batch Operations

Stages process entire windows at once, not line-by-line:

```rust
// Good: Process all lines together
fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
    for line in &mut data.lines {
        self.process_line(line);
    }
    data
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

## Related Documentation

- [Custom Stages](./custom-stages.md) - Adding custom render stages
- [UI Systems](./ui-systems.md) - Plugin UI rendering
- [Screen](../architecture/screen.md) - FrameBuffer details
