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

### 2. Tab Key Not Available for Confirm

Tab cannot be used for completion confirm due to architectural limitations in the keybinding system. The system doesn't support "fallback" behavior when a command returns `NotHandled` - once a keybinding is found, the key is consumed even if the handler doesn't process it.

**Current behavior**: Tab always inserts `\t` in insert mode (when no keybinding overrides it).

**Workaround**: Use `Ctrl+y` to confirm completion.

## Future Improvements

- [ ] LSP completion source integration
- [ ] Snippet support
- [ ] Fuzzy matching improvements
- [ ] Keybinding fallback system (for smart Tab handling)
