# Context Provider System

The Context Provider system provides a plugin-agnostic API for detecting and querying code/document scope hierarchy at a cursor position. This enables features like statusline breadcrumbs, sticky headers, and scope-aware navigation.

## Overview

**Epic #129: Context Provider Trait System**

The system consists of:
- **Core `ContextProvider` trait** - Standard interface for scope detection
- **`ContextHierarchy`** - Representation of nested scopes (outermost to innermost)
- **Provider registry** - Multi-provider support with priority-based resolution
- **Built-in providers** - Markdown headings and Treesitter AST-based detection

## Architecture

### Event-Driven Context Flow

The context system uses an event-driven architecture for efficient updates:

```
Core Events                    Context Plugin                      Consumers
─────────────────────────────────────────────────────────────────────────────
                          ┌─────────────────────────┐
BufferModified ──────────►│                         │
CursorMoved ─────────────►│    ContextPlugin        │
ViewportScrolled ────────►│                         │
                          │  - Subscribe core events│
                          │  - Query treesitter     │──► CursorContextUpdated ──► Statusline
                          │  - Cache results        │
                          │  - Compute context      │──► ViewportContextUpdated ──► StickyContext
                          │  - Emit context events  │
                          └─────────────────────────┘
```

### Unified Treesitter Parsing

The treesitter system uses a unified tree storage for both syntax highlighting and context queries:

```
┌─────────────────────────────────────────────────────────────────┐
│                      TreesitterManager                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ trees: HashMap<buffer_id, Tree>   ← single source       │   │
│  │ sources: HashMap<buffer_id, String>                     │   │
│  │ buffer_languages: HashMap<buffer_id, String>            │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
         ▲                                    ▲
         │ set_tree()                         │ get_tree()
         │                                    │
    TreeSitterSyntax                  TreesitterContextProvider
    (parse() writes)                  (get_context() reads)
```

**Key Points:**
- `TreeSitterSyntax` syncs its parse tree to `TreesitterManager` after every parse
- `TreesitterContextProvider` reads trees from the shared manager
- No duplicate parsing - both highlighting and context use the same tree

### Provider Registry

```
┌─────────────────────────────────────────────────────────────┐
│                    PluginStateRegistry                      │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ Context Provider Registry (Vec<ContextProvider>)      │  │
│  │  - TreesitterContextProvider (all languages)          │  │
│  │  - Future: LSPContextProvider, etc.                   │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
              get_context(buffer_id, line, col, content)
                           │
                           ▼
                ┌──────────────────────┐
                │  ContextHierarchy    │
                │  items: Vec<...>     │
                │  - Module            │
                │    - Impl/Class      │
                │      - Function      │
                └──────────────────────┘
```

## Core API

### ContextProvider Trait

```rust
pub trait ContextProvider: Send + Sync + 'static {
    /// Get context hierarchy at cursor position
    fn get_context(
        &self,
        buffer_id: usize,
        line: u32,
        col: u32,
        content: &str,
    ) -> Option<ContextHierarchy>;

    /// Provider name for debugging
    fn name(&self) -> &'static str;

    /// Check if this provider supports a buffer
    fn supports_buffer(&self, buffer_id: usize) -> bool;
}
```

### ContextHierarchy

```rust
pub struct ContextHierarchy {
    pub buffer_id: usize,
    pub line: u32,
    pub col: u32,
    pub items: Vec<ContextItem>,  // Outermost to innermost
}

pub struct ContextItem {
    pub text: String,        // Display text (e.g., "main", "MyClass")
    pub start_line: u32,     // Start line of this scope
    pub end_line: u32,       // End line of this scope
    pub kind: String,        // Scope type (e.g., "function", "heading", "class")
    pub level: usize,        // Nesting depth (0 = outermost)
}
```

### Helper Methods

```rust
impl ContextHierarchy {
    /// Format as breadcrumb string
    pub fn to_breadcrumb(&self) -> String;

    /// Get current (innermost) scope
    pub fn current_scope(&self) -> Option<&ContextItem>;

    /// Get scope at specific nesting level
    pub fn at_level(&self, level: usize) -> Option<&ContextItem>;
}
```

## Built-in Providers

### MarkdownContextProvider (Issue #131)

Detects heading hierarchy in markdown files using tree-sitter parsing.

**Features:**
- Supports H1-H6 ATX-style headings (`#`, `##`, `###`, etc.)
- Smart content-based caching (>95% cache hit rate)
- Stack-based hierarchy algorithm for nested headings
- Returns breadcrumb format: `> CLAUDE.md > Architecture > Workspace Structure`

**Example:**
```markdown
# Architecture
## Core Components
### Buffer Management

cursor here -> returns: ["Architecture", "Core Components", "Buffer Management"]
```

### TreesitterContextProvider (Issue #132)

Detects code scope hierarchy using AST traversal via tree-sitter.

**Supported Languages:**
- Rust: modules, impl blocks, functions, closures
- Python: classes, functions, methods
- JavaScript/TypeScript: classes, functions, methods
- C/C++: functions, classes, namespaces
- Go: functions, methods, interfaces
- Java: classes, methods

**Features:**
- Multi-language support through tree-sitter queries
- AST-based scope detection (accurate nesting)
- Handles complex nesting (e.g., closure in method in impl)

**Example:**
```rust
impl Screen {
    fn render_windows(&self) {
        // cursor here -> returns: ["Screen", "render_windows"]
    }
}
```

## Usage in Plugins

### Querying Context

```rust
// In any plugin handler
fn my_handler(ctx: &HandlerContext, state: &PluginStateRegistry) {
    let buffer_id = /* get active buffer */;
    let content = /* get buffer content */;
    let (line, col) = /* get cursor position */;

    // Query context from registered providers
    if let Some(hierarchy) = state.get_context(buffer_id, line, col, &content) {
        let breadcrumb = hierarchy.to_breadcrumb();
        println!("Current context: {}", breadcrumb);

        // Access individual scopes
        if let Some(current) = hierarchy.current_scope() {
            println!("Current scope: {} ({})", current.text, current.kind);
        }
    }
}
```

### Registering a Provider

```rust
// In your plugin's init_state()
fn init_state(&self, registry: &PluginStateRegistry) {
    let provider = Arc::new(MyContextProvider::new());
    registry.register_context_provider(provider);
}
```

## Real-World Use Cases

### 1. Statusline Breadcrumb (Issue #133)

Shows current scope in statusline: ` > File > Class > Method > `

**Implementation:**
- `StatuslineRenderContext` extended with `buffer_content`, `active_buffer_id`, `cursor_row`, and `cursor_col`
- Context section queries provider at actual cursor position
- Smart truncation for long names/deep nesting
- Settings: `context_breadcrumb_enabled`, `context_separator`, `context_max_items`

### 2. Scope Navigation Commands (Issue #133)

Jump between scopes with keyboard shortcuts:
- `gu` - Jump to parent scope (go up)
- `[s` - Jump to previous scope header
- `]s` - Jump to next scope header

**Implementation:**
- Commands registered in treesitter plugin
- Uses `RequestCursorMove` event to jump to scope start lines
- Works across all languages with context providers

### 3. Sticky Headers (Issue #88)

Display enclosing scope headers at viewport top (like VS Code's sticky scroll).

**Implementation:**
- `ContextPlugin` subscribes to `ViewportScrolled` events
- On scroll, queries `TreesitterContextProvider` for context at viewport top line
- Emits `ViewportContextUpdated` event with context hierarchy
- `StickyContextPlugin` subscribes and renders header overlay

**Architecture:**
```
ViewportScrolled ──► ContextPlugin ──► ViewportContextUpdated ──► StickyContextPlugin
                          │                                              │
                          ▼                                              ▼
              TreesitterContextProvider                         Render overlay window
                (get_context at top_line)                       with scope headers
```

## Performance Considerations

### Caching Strategy

**MarkdownContextProvider:**
- Content-based cache using hash comparison
- Cache hit rate: >95% (only re-parse on actual content changes)
- Cache invalidation: automatic on buffer modification

**TreesitterContextProvider:**
- Leverages existing treesitter parse tree (already cached)
- Incremental parsing for edits
- Query execution is fast (<1ms for typical files)

### Optimization Tips

1. **Limit query frequency** - Only query on cursor move, not every render
2. **Reuse results** - Cache hierarchy in plugin state if querying frequently
3. **Async updates** - For UI features, query in background and update async
4. **Smart invalidation** - Only re-query when cursor changes scope

## Testing

### Unit Testing Providers

```rust
#[test]
fn test_nested_scopes() {
    let provider = MyContextProvider::new();
    let content = "..."; // Test content
    let hierarchy = provider.get_context(0, 10, 5, content).unwrap();

    assert_eq!(hierarchy.items.len(), 3);
    assert_eq!(hierarchy.items[0].text, "OuterScope");
    assert_eq!(hierarchy.items[1].text, "MiddleScope");
    assert_eq!(hierarchy.items[2].text, "InnerScope");
}
```

### Integration Testing

Test with real markdown/code files to verify correct hierarchy detection at various cursor positions.

## Future Extensions

### Planned Providers

- **LSPContextProvider** - Query LSP for document symbols (Issue #88)
- **FoldContextProvider** - Use fold ranges as scope boundaries
- **CommentContextProvider** - Parse structured comments (e.g., `// MARK:` in Swift)

### API Extensions

- `get_context_at_line()` - Query without column (for sticky headers)
- `get_all_scopes()` - Return all scopes in buffer (for outline view)
- `get_sibling_scopes()` - Navigate to next/prev scope at same level

## References

- Epic: [#129 Context Provider Trait System](https://github.com/ds1sqe/reovim/issues/129)
- Core trait: [#130 Core Trait & Registry](https://github.com/ds1sqe/reovim/issues/130)
- Markdown: [#131 Markdown Provider](https://github.com/ds1sqe/reovim/issues/131)
- Treesitter: [#132 Treesitter Provider](https://github.com/ds1sqe/reovim/issues/132)
- Statusline: [#133 Statusline Integration](https://github.com/ds1sqe/reovim/issues/133)
- Sticky Headers: [#88 Sticky Headers Overlay](https://github.com/ds1sqe/reovim/issues/88)
