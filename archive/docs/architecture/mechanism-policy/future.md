# Future Directions

## Current State (v0.14.5-dev)

Mechanism/policy separation complete for:

- Keymap system (Epic #353)
- User configuration (keymap.toml)

## Planned Extensions

### Alternative Keybinding Policies

**Shipped:**

| Module | Description |
|--------|-------------|
| `modules/emacs/` | Emacs-style keybindings with Ctrl/Meta chords |

**Planned:**

| Module | Description |
|--------|-------------|
| `modules/kakoune/` | Select-then-operate model |
| `modules/helix/` | Helix-style selection and editing |

### Other Domains (Future Epics)

| Domain | Mechanism | Policy Options |
|--------|-----------|----------------|
| **Git** | `git:status()` -> facts | Fugitive vs Lazygit vs Magit |
| **LSP** | `lsp:complete()` -> items | CoC vs nvim-lsp ranking |
| **AI** | `ai:suggest()` -> text | Copilot vs Claude display |
| **Snippets** | `snippets:expand()` -> template | UltiSnips vs LuaSnip |
| **Diagnostics** | `diag:list()` -> [Diagnostic] | ALE vs nvim-lint display |

### Runtime Policy Swapping

Future configuration could allow runtime resolver swapping:

```toml
# Future: ~/.config/reovim/init.toml
[resolvers]
normal = "eager"  # Swap Vim for immediate execution
insert = "vim"    # Keep Vim behavior in insert mode
tetromino = "eager"  # Game modes always eager
```

### Per-Filetype Policies

```toml
# Future: different behavior per language
[filetypes.markdown]
resolver = "prose"  # Prose-friendly keybindings

[filetypes.rust]
resolver = "vim"    # Standard Vim

[filetypes.game]
resolver = "eager"  # Instant response
```

## API Stability

The `query()` API is designed to be stable:

- `KeyLookupState` enum may gain variants (with deprecation)
- `ModeKeyResolver` trait is the extension point
- Layered bindings (`BindingLayer`) may gain layers

## Contributing New Policies

To add a new keybinding policy:

1. Create `modules/your-policy/`
2. Implement `ModeKeyResolver` for each mode
3. Register bindings at `BindingLayer::Policy`
4. Document the policy's behavior

The mechanism layer (server + drivers) should not need changes.

## See Also

- [Vision](vision.md) - The Policy-Composable Editor promise
- [Keymap Implementation](keymap.md) - Current implementation details
- [Mechanism vs Policy Philosophy](../../contributing/philosophy/mechanism-vs-policy.md) - Foundation
