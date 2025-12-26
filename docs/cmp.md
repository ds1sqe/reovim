# Completion Plugin

Auto-completion system with background processing and extensible source architecture.

## Keybindings (Insert Mode)

| Key | Action |
|-----|--------|
| `Alt+Space` | Trigger completion popup |
| `Ctrl+n` | Select next item |
| `Ctrl+p` | Select previous item |
| `Ctrl+y` | Confirm selection |

## Architecture

- **Saturator**: Background task for non-blocking completion computation
- **Cache**: Lock-free ArcSwap cache for responsive UI rendering
- **Sources**: Extensible via `SourceSupport` trait

### Built-in Sources

- `BufferWordsSource`: Completes from words in current buffer

## Known Issues

### 1. Keybinding Conflicts (Critical)

`Ctrl+y` confirm keybinding may conflict with other plugins (e.g., telescope) depending on the current mode state. When completion popup is visible but another plugin intercepts `Ctrl+y`, the completion confirm won't work.

**Workaround**: Ensure you're in insert mode with editor focus when confirming.

### 2. Ghost Text Position (Visual)

Ghost text (inline preview) renders at incorrect position:
- Does not account for line number column width
- Does not account for left panel offset (e.g., explorer)
- Buffer scroll position not considered

The ghost text may overlay existing text instead of appearing after the cursor.

### 3. Prefix Extraction (Functional)

Word prefix extraction has issues:
- May include preceding word if no space separator (e.g., "barhel" instead of "hel")
- Does not handle all word boundary characters correctly

### 4. Tab Key Not Available for Confirm

Tab in insert mode is hardcoded to insert `\t` character before keybinding lookup. This prevents using Tab for completion confirm without breaking normal tab insertion.

### 5. Popup Position (Visual)

Popup window position uses buffer coordinates directly without:
- Line number column offset
- Window anchor/scroll offset
- Left panel (explorer) offset

May render at wrong position when these offsets apply.

### 6. No Fallback for Unhandled Confirm

When `CompletionConfirm` is triggered but completion is not active, the event is consumed but nothing happens. Should fall back to default behavior (e.g., insert newline for Enter, insert tab for Tab).

## Future Improvements

- [ ] LSP completion source integration
- [ ] Snippet support
- [ ] Fuzzy matching improvements
- [ ] Fix ghost text coordinate calculation with line number offset
- [ ] Smart Tab handling (confirm when completion active, insert tab otherwise)
