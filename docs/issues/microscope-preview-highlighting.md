# Microscope Preview Syntax Highlighting

## Description
The microscope preview panel shows file content but without syntax highlighting. The infrastructure (StyledSpan, styled_lines) is in place but treesitter integration is not implemented.

## Current State
- `PreviewContent` has `styled_lines: Option<Vec<Vec<StyledSpan>>>` field
- `render_preview_panel` checks for styled_lines and applies per-character styles
- Pickers set `styled_lines: None` - no highlights generated

## What's Needed
1. Add treesitter dependency to pickers crate or microscope plugin
2. In picker `preview()` method:
   - Get language from file extension
   - Parse content with tree-sitter
   - Run highlight query
   - Convert to StyledSpan per line
3. Pass styled_lines to PreviewContent

## Files to Modify
- `plugins/features/pickers/Cargo.toml` - Add treesitter dep
- `plugins/features/pickers/src/files.rs` - Generate highlights
- `plugins/features/pickers/src/grep.rs` - Generate highlights
- `plugins/features/pickers/src/recent.rs` - Generate highlights

## Alternative Approach
Do highlighting in microscope plugin instead of pickers:
- Microscope receives PreviewContent with syntax field
- Microscope uses LanguageRegistry to get treesitter config
- Microscope generates styled_lines before rendering

## Priority
Medium - improves user experience but preview is functional without it.
