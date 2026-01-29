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

Tree-sitter integration is in the syntax driver:

```
server/lib/drivers/syntax/      # SyntaxDriver with tree-sitter
```

Language-specific queries are bundled with the driver. This keeps the kernel free of tree-sitter dependencies while providing syntax highlighting capabilities.

## Related Documents

- [Driver Overview](../overview.md) - Driver layer architecture
