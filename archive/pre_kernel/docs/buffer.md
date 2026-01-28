# Buffer

Text storage with cursor, selection, and syntax.

## Module Location

```
lib/core/src/buffer/
```

## Buffer Struct

```rust
pub struct Buffer {
    pub id: usize,
    pub cur: Position,
    pub contents: Vec<Line>,
    pub selection: Selection,
    pub file_path: Option<String>,
    pub history: UndoHistory,
    syntax: Option<Box<dyn SyntaxProvider>>,  // Buffer-owned syntax state
}
```

## Traits

- `TextOps` - insert, delete, content manipulation
- `SelectionOps` - visual mode selection
- `CursorOps` - word navigation

## Syntax Methods

- `attach_syntax()` - Attach syntax provider (called by runtime on file open)
- `syntax()` - Get immutable reference to syntax provider
- `syntax_mut()` - Get mutable reference for incremental parsing

## Syntax System

Buffer-centric syntax highlighting architecture inspired by Helix. Each buffer owns its syntax state.

### Core Traits (no tree-sitter dependency)

```rust
/// Abstract syntax provider for buffer highlighting
pub trait SyntaxProvider: Send + Sync {
    fn language_id(&self) -> &str;
    fn highlight_range(&self, content: &str, start: u32, end: u32) -> Vec<Highlight>;
    fn parse(&mut self, content: &str);
    fn parse_incremental(&mut self, content: &str, edit: &EditInfo);
    fn is_parsed(&self) -> bool;
}

/// Factory for creating syntax providers
pub trait SyntaxFactory: Send + Sync {
    fn create_syntax(&self, file_path: &str, content: &str) -> Option<Box<dyn SyntaxProvider>>;
    fn supports_file(&self, file_path: &str) -> bool;
}
```

### Buffer Integration

```rust
pub struct Buffer {
    // ...existing fields...
    syntax: Option<Box<dyn SyntaxProvider>>,
}

impl Buffer {
    pub fn attach_syntax(&mut self, syntax: Box<dyn SyntaxProvider>);
    pub fn syntax(&self) -> Option<&dyn SyntaxProvider>;
    pub fn syntax_mut(&mut self) -> Option<&mut dyn SyntaxProvider>;
}
```

### Data Flow

```
1. File Opened
   └── Runtime detects language via SyntaxFactory
       └── Creates SyntaxProvider (e.g., TreeSitterSyntax)
           └── Attaches to buffer.syntax

2. Buffer Edit
   └── buffer.syntax_mut().parse_incremental(content, edit)

3. Render
   └── RenderData::from_buffer()
       └── buffer.syntax().highlight_range(content, start, end)
           └── Highlights applied to framebuffer
```

### TreeSitterSyntax (in plugin)

```rust
// plugins/features/treesitter/src/syntax.rs
pub struct TreeSitterSyntax {
    language_id: String,
    parser: Parser,
    tree: Option<Tree>,
    query: Arc<Query>,  // Pre-compiled highlights query
    highlighter: Highlighter,
}

impl SyntaxProvider for TreeSitterSyntax {
    fn highlight_range(&self, content: &str, start: u32, end: u32) -> Vec<Highlight> {
        // Use tree-sitter to generate highlights
    }
    // ...
}
```

### Benefits

- **Clear ownership**: Buffer owns its syntax state
- **No HashMap lookups**: Direct access via `buffer.syntax()`
- **Automatic cleanup**: Syntax dropped when buffer dropped
- **Extensible**: Easy to add other backends (regex, LSP, etc.)

## Related Documentation

- [Runtime](./runtime.md) - Buffer management
- [Screen](./screen.md) - Rendering buffers
- [Syntax Highlighting](../features/syntax-highlighting.md) - Treesitter details
