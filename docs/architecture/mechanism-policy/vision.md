# Vision: Policy-Composable Editor

Every behavior is determined by swappable policy modules.
Every driver provides pure mechanisms (facts, not decisions).
Users compose their editor by choosing policies for each domain.

```
╔═════════════════════════════════════════════════════════════════════════════════╗
║                                USER'S EDITOR                                    ║
╠═════════════════════════════════════════════════════════════════════════════════╣
║                                                                                 ║
║  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐      ║
║  │ ◆ Vim     │  │ ◆ Lazygit │  │ ◆ nvim-lsp│  │ ◆ Claude  │  │ ◆ Custom  │      ║
║  │   Keys    │  │   Policy  │  │   Policy  │  │   AI      │  │   Plugin  │      ║
║  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘      ║
║        │              │              │              │              │            ║
║        ▼              ▼              ▼              ▼              ▼            ║
║  ╔═══════════════════════════════════════════════════════════════════════════╗  ║
║  ║                                                                           ║  ║
║  ║   Keymap       Git         LSP          AI        Extension               ║  ║
║  ║   Driver       Driver      Driver       Driver    Driver                  ║  ║
║  ║   (facts)      (facts)     (facts)      (facts)   (facts)                 ║  ║
║  ║                                                                           ║  ║
║  ╚═══════════════════════════════════════════════════════════════════════════╝  ║
║                                                                                 ║
║                         M E C H A N I S M   L A Y E R                           ║
║                         (stable · pure · universal)                             ║
║                                                                                 ║
╚═════════════════════════════════════════════════════════════════════════════════╝


  Another user might choose:

  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐
  │ ◇ Emacs   │  │ ◇ Fugitive│  │ ◇ CoC     │  │ ◇ Copilot │  │ ◇ Other   │
  │   Keys    │  │   Policy  │  │   Policy  │  │   AI      │  │   Plugin  │
  └───────────┘  └───────────┘  └───────────┘  └───────────┘  └───────────┘

  Same mechanisms, different behaviors.
```

## Extensibility Domains

```
┌────────────────┬────────────────────────────────┬────────────────────────────────┐
│    Domain      │     Mechanism (Driver)         │      Policy (Module)           │
├────────────────┼────────────────────────────────┼────────────────────────────────┤
│                │                                │                                │
│  Keybindings   │  query() → KeyLookupState      │  VimPolicy: wait for dd        │
│                │  "d exists, dd exists"         │  EmacsPolicy: immediate        │
│                │                                │                                │
├────────────────┼────────────────────────────────┼────────────────────────────────┤
│                │  status() → [ChangedFile]      │  FugitivePolicy: gutter        │
│  Git           │  diff() → DiffHunks            │  LazygitPolicy: popup          │
│                │  blame() → BlameInfo           │  MagitPolicy: buffer           │
│                │                                │                                │
├────────────────┼────────────────────────────────┼────────────────────────────────┤
│                │  completions() → [Item]        │  CocPolicy: popup+rank         │
│  LSP           │  diagnostics() → [Diag]        │  NvimLspPolicy: inline         │
│                │  hover() → HoverInfo           │  CustomPolicy: minimal         │
│                │                                │                                │
├────────────────┼────────────────────────────────┼────────────────────────────────┤
│                │  suggest(ctx) → Suggestion     │  CopilotPolicy: ghost          │
│  AI            │  explain(code) → Explanation   │  ClaudePolicy: panel           │
│                │  refactor(code) → NewCode      │  CustomPolicy: diff view       │
│                │                                │                                │
├────────────────┼────────────────────────────────┼────────────────────────────────┤
│  Syntax        │  tree() → SyntaxTree           │  TreesitterPolicy: highlight   │
│                │  symbols() → [Symbol]          │  SemanticPolicy: LSP highlight │
│                │                                │                                │
├────────────────┼────────────────────────────────┼────────────────────────────────┤
│  UI            │  render() → Cells              │  TUIPolicy: terminal           │
│                │                                │  GUIPolicy: native             │
│                │                                │  WebPolicy: browser            │
└────────────────┴────────────────────────────────┴────────────────────────────────┘

                 Driver → FACTS → Policy → BEHAVIOR → User → RESULT
```

## The Composability Promise

```lua
-- User config (~/.config/reovim/init.lua or similar):

policies = {
    keymap = "vim",           -- or "emacs", "kakoune", "custom"
    git = "lazygit",          -- or "fugitive", "magit", "none"
    lsp = "nvim-lsp",         -- or "coc", "ale", "none"
    ai = "claude",            -- or "copilot", "codeium", "none"
    theme = "catppuccin",     -- or "gruvbox", "tokyonight"
}

-- Mix and match freely:
-- Vim keys + Magit git + CoC LSP + Claude AI
-- Emacs keys + Fugitive git + nvim-lsp + Copilot
-- Kakoune keys + Lazygit + Custom LSP + No AI
```

## Non-Vim Applications

This architecture enables completely different applications using the same engine.

### Example: TetrisGame Mode

```rust
// modules/tetris/src/lib.rs (NOT part of Epic #353 - vision only)

pub struct TetrisModule;

impl Module for TetrisModule {
    fn name(&self) -> &str { "tetris" }

    fn init(&self, ctx: &mut ModuleContext) {
        // Register TetrisResolver - needs INSTANT response
        ctx.register_resolver(ModeId::from("tetris"), Arc::new(TetrisResolver));

        // Register bindings at Policy layer
        ctx.register_binding_at_layer(BindingLayer::Policy, Binding {
            mode: "tetris",
            keys: "<Left>",
            command: "tetris:move_left",
        });
        // Arrow keys, Space for drop, z/x for rotate...
    }
}

/// Tetris needs INSTANT response - no waiting for longer sequences
struct TetrisResolver;

impl ModeKeyResolver for TetrisResolver {
    fn resolve(&self, ctx: &ResolveContext) -> ResolveResult {
        let state = ctx.registry.query(ctx.mode, ctx.keys);

        match state {
            // EAGER: Execute immediately, don't wait!
            KeyLookupState::ExactWithLonger { exact } => ResolveResult::Execute(exact),
            KeyLookupState::ExactOnly(cmd) => ResolveResult::Execute(cmd),
            KeyLookupState::PrefixOnly => ResolveResult::NeedMoreKeys,
            KeyLookupState::NotFound => ResolveResult::Ignore,  // Don't beep
        }
    }
}
```

### Example: ChessGame Mode

```rust
// modules/chess/src/lib.rs (NOT part of Epic #353 - vision only)

/// Chess resolver - parses algebraic notation
struct ChessResolver;

impl ModeKeyResolver for ChessResolver {
    fn resolve(&self, ctx: &ResolveContext) -> ResolveResult {
        let input = ctx.keys.to_string();

        // Try to parse as chess move
        match parse_algebraic_notation(&input) {
            ParseResult::Complete(chess_move) => {
                // Valid move like "e4", "Nf3", "O-O"
                ResolveResult::ExecuteWithArgs(
                    cmd("chess:move"),
                    vec![ArgValue::String(chess_move)],
                )
            }
            ParseResult::Incomplete => {
                // Partial input like "e" or "Nf" - wait for more
                ResolveResult::NeedMoreKeys
            }
            ParseResult::Invalid => {
                // Not a valid chess notation
                ResolveResult::NotFound
            }
        }
    }
}
```

### Why This Works

```
┌──────────────────┬─────────────────────────┬─────────────────────────────────┐
│      Mode        │       Resolver          │         Behavior                │
├──────────────────┼─────────────────────────┼─────────────────────────────────┤
│  normal (Vim)    │  VimNormalResolver      │  Wait for `dd` after `d`        │
│  insert (Vim)    │  VimInsertResolver      │  Pass keys to buffer            │
│  tetris          │  TetrisResolver         │  Execute instantly              │
│  chess           │  ChessResolver          │  Parse algebraic notation       │
└──────────────────┴─────────────────────────┴─────────────────────────────────┘

       Same engine, same registry, same query()
       Different resolvers = Different behavior
```

## See Also

- [Mechanism vs Policy Philosophy](../../contributing/philosophy/mechanism-vs-policy.md) - Unix philosophy origins
- [Keymap Implementation](keymap.md) - How we achieved this for keybindings
- [Layer Architecture](layers.md) - Where code belongs
