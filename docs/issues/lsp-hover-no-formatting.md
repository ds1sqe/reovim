# LSP Hover - No Syntax Highlighting or Markdown Rendering

## Status: NOT FIXED

## Problem

LSP hover popups display raw text without any formatting:
- **No syntax highlighting** for code blocks
- **No markdown rendering** for documentation
- Code appears as plain monochrome text in a box

## Example

When hovering over a Rust struct, the popup shows:

```
```rust
reovim_plugin_completion::cache
```

```rust
pub struct CompletionSnapshot {
    pub items: Vec<CompletionItem>,
    pub prefix: String,
    pub buffer_id: usize,
    pub cursor_row: u32,
    pub cursor_col: u32,
    /* … */
}
```
```

But it's rendered as plain white text on dark background with no syntax highlighting.

## Expected Behavior

The hover popup should:
1. **Parse markdown** from LSP hover responses
2. **Apply syntax highlighting** to code blocks using treesitter
3. **Render markdown elements**:
   - Code blocks with language-specific highlighting
   - Bold/italic text
   - Links
   - Headers

## Current Implementation

**File**: `plugins/features/lsp/src/window.rs`

The hover window renders content but doesn't process markdown:
- Line 93-111: Renders plain text line by line
- No markdown parsing
- No syntax highlighting

**File**: `plugins/features/lsp/src/hover.rs`

The hover cache stores raw content:
- `HoverSnapshot.content` is a plain `String`
- No decoration or parsing applied

## Root Cause

The hover implementation was designed to support markdown (based on code comments mentioning `MarkdownDecorator::parse_decorations()`), but:

1. **Markdown parsing not implemented** in hover window rendering
2. **Treesitter integration missing** for code blocks
3. **Content stored as raw string** without processing

## Related Code

**Hover Response Handling** (`plugins/features/lsp/src/lib.rs:441-545`):
```rust
// Extract hover content from LSP response
let content = match hover.contents {
    HoverContents::Scalar(marked) => extract_markdown_text(&marked),
    HoverContents::Array(items) => {
        items.iter().map(|m| extract_markdown_text(m)).collect::<Vec<_>>().join("\n\n")
    }
    HoverContents::Markup(markup) => markup.value,
};
```

The content is extracted but not parsed.

**Hover Window Rendering** (`plugins/features/lsp/src/window.rs:93-111`):
```rust
// Render hover content
for (line_idx, line) in snapshot.lines().enumerate() {
    // ... renders plain text ...
}
```

## Solution Approach

### Option 1: Use Existing Markdown Plugin

Reuse the markdown language plugin for rendering:

1. Parse hover content with `MarkdownDecorator::parse_decorations()`
2. Apply decorations (bold, italic, code)
3. Use treesitter for code block syntax highlighting
4. Render decorated content in hover window

### Option 2: Implement Minimal Markdown Support

Create a lightweight markdown renderer for hover:

1. Detect code blocks with language tags (` ```rust `)
2. Apply treesitter highlighting per language
3. Render bold (`**text**`) and italic (`*text*`)
4. Ignore other markdown elements

### Recommended: Option 1

Use existing infrastructure:
- `plugins/languages/markdown/` - Markdown treesitter support
- `lib/core/src/decoration/` - Decoration system
- Consistent with how markdown files are rendered

## Implementation Steps

1. **Parse markdown in hover handler** (`lib.rs`):
   - Use `MarkdownDecorator::parse_decorations()` on hover content
   - Store decorations alongside content in `HoverSnapshot`

2. **Update HoverSnapshot** (`hover.rs`):
   ```rust
   pub struct HoverSnapshot {
       pub content: String,
       pub decorations: Vec<Decoration>,  // NEW
       // ...
   }
   ```

3. **Render with decorations** (`window.rs`):
   - Apply decoration styles when rendering each character
   - Use treesitter highlighting for code blocks

4. **Handle code blocks specially**:
   - Extract language from fence (` ```rust `)
   - Parse with treesitter
   - Apply language-specific highlighting

## Files to Modify

| File | Change |
|------|--------|
| `plugins/features/lsp/src/lib.rs` | Parse markdown when hover response arrives |
| `plugins/features/lsp/src/hover.rs` | Store decorations in `HoverSnapshot` |
| `plugins/features/lsp/src/window.rs` | Render with decoration styles |

## Dependencies

- `reovim-lang-markdown` - Markdown parsing and decoration
- `reovim-plugin-treesitter` - Syntax highlighting for code blocks

## Priority

**Medium** - Hover works functionally but lacks polish. Users can read the content but it's harder to understand without highlighting.

## Related Issues

- Same issue may affect other markdown content (diagnostics, completion docs)
- Consider implementing a shared markdown renderer component

## Verification Steps

1. Build and start server
2. Trigger hover on a Rust symbol
3. Verify:
   - Code blocks have syntax highlighting
   - Bold/italic text is styled
   - Overall readability improved

## Notes

The LSP specification (3.17) supports markdown in hover responses via `MarkupContent` with `kind: "markdown"`. rust-analyzer sends rich markdown content, but reovim currently ignores the formatting.
