# syntax-treesitter/ - Tree-sitter Syntax Driver

Generic tree-sitter implementation of the SyntaxDriver trait.

## Source Location

`ext/server/drivers/syntax-treesitter/src/`

## Purpose

Provides a tree-sitter based implementation of the `SyntaxDriver` trait from
`reovim-driver-syntax`. Handles the generic tree-sitter parsing and highlighting
logic, but does NOT include any language grammars.

## Architecture

Three-layer design:

```
Language Modules                  This Crate                   Trait Crate
================                  ==========                   ===========
treesitter-rust     ---------->  TreeSitterDriver  --impl-->  SyntaxDriver
treesitter-markdown              CaptureMapper
treesitter-python
```

1. **`reovim-driver-syntax`** (trait crate): Defines `SyntaxDriver`, `HighlightGroup`
2. **`reovim-driver-syntax-treesitter`** (this crate): Generic tree-sitter implementation
3. **Language modules** (e.g., `treesitter-rust`): Provide grammars and queries

This separation keeps language-specific dependencies (tree-sitter-rust, etc.)
in their own modules rather than bundled into the driver.

## Key Types

### TreeSitterDriver

```rust
pub struct TreeSitterDriver {
    language_id: String,
    parser: Mutex<Parser>,
    tree: RwLock<Option<Tree>>,
    highlights_query: Arc<Query>,
    folds_query: Option<Arc<Query>>,
    indents_query: Option<Arc<Query>>,
    injection_manager: Option<InjectionManager>,
    capture_mapper: Arc<CaptureMapper>,
    // ...
}

impl TreeSitterDriver {
    pub fn new(
        language_id: &str,
        language: &Language,
        highlights_query: Arc<Query>,
        capture_mapper: Arc<CaptureMapper>,
    ) -> Result<Self, SyntaxError>;

    pub fn with_queries(
        self,
        folds_query: Option<Query>,
        indents_query: Option<Query>,
        injections_query: Option<Query>,
    ) -> Self;

    pub fn parse(&mut self, content: &str);
    pub fn highlights(&self, range: Range<usize>) -> Vec<HighlightSpan>;
    pub fn folds(&self) -> Vec<FoldRange>;
    pub fn indent_for(&self, line: usize) -> usize;
}
```

### CaptureMapper

Maps tree-sitter capture names to highlight groups:

```rust
pub struct CaptureMapper {
    mappings: HashMap<String, HighlightGroup>,
}

impl CaptureMapper {
    pub fn new() -> Self;
    pub fn with_mapping(mut self, capture: &str, group: HighlightGroup) -> Self;
    pub fn map(&self, capture: &str) -> Option<HighlightGroup>;
}
```

### Injection Support

For embedded language highlighting (e.g., Markdown in doc comments):

```rust
pub trait InjectionLayerFactory: Send + Sync {
    fn language_id(&self) -> &str;
    fn create(&self) -> Box<dyn SyntaxDriver>;
}

pub struct InjectionManager {
    layers: Vec<InjectionLayer>,
}

pub struct InjectionLayerStore {
    factories: RwLock<HashMap<String, Arc<dyn InjectionLayerFactory>>>,
}
```

## Thread Safety

`TreeSitterDriver` is `Send + Sync` through interior mutability:
- `Parser` and `QueryCursor` use `Mutex` (exclusive access needed)
- `Tree` and content use `RwLock` (read-heavy access pattern)

## Example Usage

```rust
use std::sync::Arc;
use reovim_driver_syntax_treesitter::{TreeSitterDriver, CaptureMapper};
use tree_sitter::Query;

// Language module provides grammar and query
let language = tree_sitter_rust::LANGUAGE;
let query = Query::new(&language.into(), RUST_HIGHLIGHTS_QUERY).unwrap();

// Create driver
let mapper = Arc::new(CaptureMapper::new()
    .with_mapping("keyword", HighlightGroup::Keyword)
    .with_mapping("function", HighlightGroup::Function));
let mut driver = TreeSitterDriver::new(
    "rust",
    &language.into(),
    Arc::new(query),
    mapper,
).unwrap();

// Parse and highlight
driver.parse("fn main() {}");
let highlights = driver.highlights(0..12);
```

## Re-exports

```rust
// Re-export tree_sitter types that language modules need
pub use tree_sitter::{Language, Query};
```

## Related Documents

- [Driver Overview](../overview.md)
- [syntax Driver](../syntax/overview.md)
