# syntax/ - Syntax Driver

Abstraction for syntax highlighting. **No tree-sitter in this crate.**

## Source Location

`lib/drivers/syntax/src/`

## Key Traits

```rust
pub trait SyntaxDriver: Send + Sync {
    fn name(&self) -> &str;
    fn extensions(&self) -> &[&str];

    fn parse(&mut self, source: &str);
    fn highlight(&self, range: Range) -> Vec<HighlightSpan>;
    fn fold_regions(&self) -> Vec<FoldRegion>;
}
```

## Highlight Groups

```rust
pub enum HighlightGroup {
    Keyword,
    Function,
    String,
    Comment,
    Type,
    Variable,
    Operator,
    // ... 120+ groups
}

pub struct HighlightSpan {
    pub range: Range,
    pub group: HighlightGroup,
}
```

## Implementation Location

Tree-sitter implementations live in plugins, not drivers:

```
plugins/features/treesitter/    # TreeSitterDriver impl
plugins/languages/rust/         # Rust queries
plugins/languages/python/       # Python queries
```

This separation keeps the driver layer free of tree-sitter dependencies.

## Related Documents

- [Driver Overview](../overview.md) - Driver layer architecture
