# Custom Render Stages

Guide to extending the render pipeline with custom stages.

## Adding Custom Stages

Plugins can register custom render stages during `build()`:

```rust
impl Plugin for MyPlugin {
    fn build(&self, ctx: &mut PluginContext) {
        // Register the render stage
        ctx.render_stages.lock().unwrap().register(Arc::new(MyCustomStage));
    }
}

struct MyCustomStage;

impl RenderStage for MyCustomStage {
    fn name(&self) -> &'static str { "my_stage" }

    fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
        // Add custom rendering logic
        // Modify data and return
        data
    }
}
```

Note: Use `Arc::new()` (not `Box::new()`) when registering stages.

## Stage Execution Order

Stages are executed in registration order. Built-in stages have these priorities:

| Stage | Priority | Description |
|-------|----------|-------------|
| Visibility | 50 | Apply folding (collapsed/hidden lines) |
| Highlighting | 100 | Treesitter syntax highlighting |
| Decorations | 120 | Language decorations (markdown headers, etc.) |
| Visual | 150 | Visual mode selection highlighting |
| Animation | 200 | Animated effects (yank, paste feedback) |
| Indent | 220 | Indent guide computation |
| Signs | 230 | Sign column (diagnostics, git, etc.) |
| VirtualText | 240 | Inline diagnostic messages |

## Example: Custom Highlight Stage

```rust
pub struct MyHighlightStage {
    pattern: Regex,
    style: Style,
}

impl RenderStage for MyHighlightStage {
    fn name(&self) -> &'static str { "my_highlight" }

    fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
        for (line_idx, line) in data.lines.iter().enumerate() {
            for m in self.pattern.find_iter(line) {
                data.highlights[line_idx].push(LineHighlight {
                    start_col: m.start(),
                    end_col: m.end(),
                    style: self.style.clone(),
                });
            }
        }
        data
    }
}
```

## Example: Custom Decoration Stage

```rust
pub struct MarkdownLinkStage;

impl RenderStage for MarkdownLinkStage {
    fn name(&self) -> &'static str { "markdown_links" }

    fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
        // Only apply to markdown files
        if ctx.filetype != "markdown" {
            return data;
        }

        for (line_idx, line) in data.lines.iter().enumerate() {
            // Find markdown links [text](url)
            // Add decorations to conceal URL part
            // ...
        }
        data
    }
}
```

## Integration Points

### Signs and Gutter

Add signs to the gutter:

```rust
fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
    // Add a sign to line 5
    data.signs[5] = Some(LineSign {
        char: '●',
        style: Style::default().fg(Color::Red),
        priority: 100,
    });
    data
}
```

### Virtual Text

Add end-of-line virtual text:

```rust
fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
    data.virtual_texts[10] = Some(VirtualTextEntry {
        line: 10,
        text: " ← error here".to_string(),
        style: Style::default().fg(Color::Red),
        priority: 100,
    });
    data
}
```

### Decorations

Add inline decorations:

```rust
fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
    // Conceal characters 5-10 on line 3
    data.decorations[3].push(Decoration {
        start_col: 5,
        end_col: 10,
        kind: DecorationKind::Conceal {
            replacement: Some("…".to_string()),
        },
    });
    data
}
```

## Best Practices

### Performance

1. **Early exit**: Return immediately if stage doesn't apply

```rust
fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
    if !self.is_relevant(ctx) {
        return data;  // Early exit
    }
    // ... expensive logic
    data
}
```

2. **Batch operations**: Process all lines together

```rust
// Good: Single pass
for line in &mut data.lines {
    self.process(line);
}

// Bad: Multiple passes
for line in &mut data.lines {
    self.highlight(line);
}
for line in &mut data.lines {
    self.decorate(line);
}
```

3. **Cache computation**: Use caches for expensive operations

```rust
struct CachingStage {
    cache: Arc<ArcSwap<ComputedData>>,
}

impl RenderStage for CachingStage {
    fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
        let cached = self.cache.load();
        // Use cached data instead of recomputing
        data
    }
}
```

### Thread Safety

- Stages must be `Send + Sync`
- Use `Arc` for shared state
- Avoid holding locks during `transform()`

### Debugging

Add tracing for debugging:

```rust
fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
    tracing::trace!(stage = self.name(), lines = data.lines.len(), "transforming");
    // ...
    data
}
```

## Related Documentation

- [Pipeline](./pipeline.md) - Pipeline stages
- [UI Systems](./ui-systems.md) - Plugin UI rendering
- [Plugin Advanced](../plugins/advanced.md) - Advanced patterns
