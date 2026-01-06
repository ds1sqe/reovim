# Decoration System

The decoration system provides language-aware visual rendering for structured documents like Markdown, org-mode, and similar formats. It allows hiding syntax markers and replacing them with visual representations.

## Overview

Decorations transform raw text into visually enhanced output:

```markdown
# Raw Markdown                    # Rendered

# Heading 1                       ◉ Heading 1
## Heading 2                      ○ Heading 2
### Heading 3                     ◦ Heading 3

- List item                       • List item
  - Nested item                     ◦ Nested item

- [ ] Unchecked                   ☐ Unchecked
- [x] Checked                     ☑ Checked

**bold text**                     bold text (styled)
*italic text*                     italic text (styled)
```

## Architecture

```
LanguageRendererRegistry
       │
       ├── MarkdownRenderer
       ├── OrgModeRenderer (future)
       └── ...

DecorationStore (per buffer)
       │
       └── Line decorations
           ├── Replacements
           ├── Conceals
           └── Virtual text
```

## Core Components

### LanguageRenderer Trait

Each language implements this trait:

```rust
pub trait LanguageRenderer: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &[&'static str];
    fn render(&self, buffer: &Buffer) -> Vec<LineDecoration>;
    fn refresh(&mut self, buffer: &Buffer);
}
```

### LanguageRendererRegistry

Dynamic dispatch for language renderers:

```rust
pub struct LanguageRendererRegistry {
    renderers: HashMap<String, Arc<dyn LanguageRenderer>>,
}

impl LanguageRendererRegistry {
    pub fn register(&mut self, renderer: Arc<dyn LanguageRenderer>);
    pub fn get_for_extension(&self, ext: &str) -> Option<Arc<dyn LanguageRenderer>>;
}
```

### DecorationStore

Per-buffer storage for computed decorations:

```rust
pub struct DecorationStore {
    decorations: HashMap<usize, LineDecorations>,  // line -> decorations
}

pub struct LineDecorations {
    pub replacements: Vec<Replacement>,
    pub conceals: Vec<ConcealRange>,
    pub virtual_text: Vec<VirtualText>,
}
```

## Decoration Types

### Replacement

Replace text with different characters:

```rust
pub struct Replacement {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub style: Style,
}
```

Example: Replace `#` with ` ◉ ` for headings.

### Conceal

Hide characters without replacement:

```rust
pub struct ConcealRange {
    pub start: usize,
    pub end: usize,
}
```

Example: Hide `**` markers around bold text.

### Virtual Text

Add text that doesn't exist in the buffer:

```rust
pub struct VirtualText {
    pub column: usize,
    pub text: String,
    pub style: Style,
}
```

Example: Add checkmark after completed tasks.

## Markdown Decorations

The built-in markdown renderer provides:

### Headings

| Syntax | Decoration |
|--------|------------|
| `#` | `◉` (level 1) |
| `##` | `○` (level 2) |
| `###` | `◦` (level 3) |
| `####` | `▸` (level 4+) |

### Lists

| Syntax | Decoration |
|--------|------------|
| `-` or `*` | `•` |
| Nested `-` | `◦` |
| `1.` | Number with styling |

### Task Lists

| Syntax | Decoration |
|--------|------------|
| `- [ ]` | `☐` (unchecked) |
| `- [x]` | `☑` (checked, green) |

### Emphasis

| Syntax | Decoration |
|--------|------------|
| `**bold**` | Hide `**`, apply bold |
| `*italic*` | Hide `*`, apply italic |
| `` `code` `` | Hide backticks, apply code style |

### Links

| Syntax | Decoration |
|--------|------------|
| `[text](url)` | Show text with link style, hide URL |

## Insert Mode Behavior

Decorations are disabled on the cursor line in insert mode:

```rust
// In render pipeline
if mode == Mode::Insert && line == cursor_line {
    skip_decorations = true;
}
```

This allows seeing raw syntax while typing.

## Integration with Render Pipeline

Decorations integrate via `DecorationRenderStage`:

```rust
impl RenderStage for DecorationRenderStage {
    fn name(&self) -> &'static str { "decorations" }
    fn priority(&self) -> u32 { 120 }  // After highlighting

    fn execute(&self, data: &mut RenderData, ctx: &RenderContext) {
        let renderer = registry.get_for_extension(ctx.extension);
        if let Some(renderer) = renderer {
            let decorations = renderer.render(ctx.buffer);
            apply_decorations(data, decorations);
        }
    }
}
```

## Creating Custom Language Renderers

### Implement the Trait

```rust
pub struct MyLanguageRenderer;

impl LanguageRenderer for MyLanguageRenderer {
    fn name(&self) -> &'static str { "my_language" }

    fn extensions(&self) -> &[&'static str] {
        &["mylang", "ml"]
    }

    fn render(&self, buffer: &Buffer) -> Vec<LineDecoration> {
        let mut decorations = Vec::new();

        for (line_num, line) in buffer.lines.iter().enumerate() {
            if line.starts_with(">>>") {
                decorations.push(LineDecoration {
                    line: line_num,
                    replacement: Some(Replacement {
                        start: 0,
                        end: 3,
                        text: "▶".to_string(),
                        style: Style::default().fg(Color::Green),
                    }),
                    ..Default::default()
                });
            }
        }

        decorations
    }

    fn refresh(&mut self, buffer: &Buffer) {
        // Update internal state if needed
    }
}
```

### Register the Renderer

```rust
impl Plugin for MyLanguagePlugin {
    fn register(&self, registry: &mut LanguageRendererRegistry) {
        registry.register(Arc::new(MyLanguageRenderer));
    }
}
```

## Treesitter Integration

Decorations can use treesitter queries for accurate parsing:

```rust
impl LanguageRenderer for MarkdownRenderer {
    fn render(&self, buffer: &Buffer) -> Vec<LineDecoration> {
        // Use treesitter for accurate parsing
        let tree = self.parser.parse(buffer.text(), None)?;
        let query = Query::new(language, DECORATION_QUERY)?;

        let mut cursor = QueryCursor::new();
        for match_ in cursor.matches(&query, tree.root_node(), buffer.text()) {
            // Process matches into decorations
        }
    }
}
```

### Decoration Queries

Create `.scm` files for decoration rules:

```scheme
;; markdown_decorations.scm

;; Headings
(atx_heading (atx_h1_marker) @heading.marker.1)
(atx_heading (atx_h2_marker) @heading.marker.2)

;; Lists
(list_item (list_marker_minus) @list.marker)
(list_item (list_marker_star) @list.marker)

;; Task lists
(task_list_marker_unchecked) @task.unchecked
(task_list_marker_checked) @task.checked

;; Emphasis
(emphasis) @emphasis
(strong_emphasis) @strong
```

## Caching

### DecorationCache

Caches computed decorations per buffer:

```rust
pub struct DecorationCache {
    cache: HashMap<BufferId, CachedDecorations>,
}

struct CachedDecorations {
    decorations: Vec<LineDecoration>,
    version: u64,  // Buffer version when computed
}
```

### Cache Invalidation

Cache is invalidated when:
- Buffer content changes
- File is saved
- Language mode changes

```rust
impl DecorationCache {
    pub fn invalidate(&mut self, buffer_id: BufferId) {
        self.cache.remove(&buffer_id);
    }

    pub fn get_or_compute(&mut self, buffer: &Buffer) -> &[LineDecoration] {
        if self.is_stale(buffer) {
            self.recompute(buffer);
        }
        &self.cache[&buffer.id].decorations
    }
}
```

## Performance Considerations

### Incremental Updates

For large files, compute decorations incrementally:

```rust
fn render_incremental(
    &self,
    buffer: &Buffer,
    changed_lines: &[usize],
) -> Vec<LineDecoration> {
    // Only recompute affected lines
    let mut decorations = self.cached_decorations.clone();
    for line in changed_lines {
        decorations[line] = self.compute_line_decoration(buffer, line);
    }
    decorations
}
```

### Lazy Computation

Only compute decorations for visible lines:

```rust
fn render_visible(
    &self,
    buffer: &Buffer,
    viewport: Range<usize>,
) -> Vec<LineDecoration> {
    buffer.lines[viewport]
        .iter()
        .enumerate()
        .map(|(i, line)| self.compute_line_decoration(line, viewport.start + i))
        .collect()
}
```

## Source Files

- `lib/core/src/decoration/mod.rs` - Core types and exports
- `lib/core/src/decoration/types.rs` - Decoration types
- `lib/core/src/decoration/provider.rs` - LanguageRenderer trait
- `lib/core/src/decoration/registry.rs` - Renderer registration
- `lib/core/src/decoration/cache.rs` - Decoration caching
- `plugins/languages/markdown/src/decorator.rs` - Markdown implementation
