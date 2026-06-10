# Mechanism/Policy Separation

Architecture documentation for Epic #353: implementing pure mechanism/policy separation in Reovim's keymap system.

## Documents

| File | Audience | Purpose |
|------|----------|---------|
| [vision.md](vision.md) | Architects | The Policy-Composable Editor promise |
| [layers.md](layers.md) | All contributors | Where code belongs, layer responsibilities |
| [keymap.md](keymap.md) | Keymap developers | Implementation details (achieved state) |
| [violations.md](violations.md) | All contributors | Anti-patterns with before/after examples |
| [future.md](future.md) | Planners | Roadmap for Git, LSP, AI domains |

## Quick Links

- [Philosophy Guide](../../contributing/philosophy/mechanism-vs-policy.md) - Unix origins and foundational principles
- [User Keymap Configuration](../../user-guide/keymap-config.md) - End-user guide for customization

## Summary

Epic #353 achieved complete separation between:

- **Mechanism (Server)**: `KeymapRegistry.query()` returns pure facts about what exists
- **Policy (Vim Module)**: `ModeKeyResolver` decides what those facts mean

This enables swappable keybinding paradigms (Vim, Emacs, Kakoune, games) using the same underlying engine.
