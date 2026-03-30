# Feature Parity: v0.8.1 vs Current

Audit comparing the pre-kernel v0.8.1 archive ([archive/pre_kernel](https://github.com/ds1sqe/reovim/tree/81806439/archive/pre_kernel)) against
the current codebase (v0.14.0). Created for issue #652, updated for #653.

Status key:
- **Present** — Feature exists with equivalent or better behavior
- **Improved** — Feature reimplemented with better architecture
- **Missing** — Feature not yet reimplemented
- **Changed** — Feature exists but behavior differs
- **New** — Feature did not exist in v0.8.1

## 1. Core Editing

| Feature | v0.8.1 | Current | Status |
|---------|--------|---------|--------|
| Buffer operations (insert, delete, replace) | Yes | Yes | Present |
| Undo/redo | Yes | Yes | Improved (per-buffer, disk-persistent) |
| Registers (unnamed, named a-z, numbered 0-9) | Yes | Yes | Present |
| System clipboard (+, * registers) | Yes | Yes | Present |
| Marks (ma, 'a, `a) | Partial | Yes | Improved (SetMark, GotoMarkLine, GotoMarkExact, #654) |
| Jump list (Ctrl-O, Ctrl-I) | Yes | Yes | Present (per-client Jumplist, push-on-jump, #654) |
| Dot repeat (.) | Yes | Yes | Present |
| Replace mode (R) | No | No | N/A |

## 2. Vim Motions

| Motion | v0.8.1 | Current | Status |
|--------|--------|---------|--------|
| h, j, k, l (character/line) | Yes | Yes | Present |
| w, b, e (word) | Yes | Yes | Present |
| W, B, E (WORD) | No | Yes | New |
| ge, gE (word end backward) | No | Yes | New |
| 0, $, ^ (line position) | Yes (0, $) | Yes (0, $, ^) | Improved |
| gg, G (document start/end) | Yes | Yes | Present |
| f, F, t, T (find char) | No | Yes | New |
| ;, , (find char repeat) | No | Yes | New |
| /, ?, n, N (search) | Yes | Yes | Present |
| *, # (word search) | Yes | Yes | Present |
| % (matching bracket) | Yes | Yes | Present |
| gj, gk (display lines) | No | Yes | New |
| H, M, L (screen position) | No | No | Missing in both |

## 3. Vim Operators

| Operator | v0.8.1 | Current | Status |
|----------|--------|---------|--------|
| d (delete) | Yes | Yes | Present |
| c (change) | Yes | Yes | Present |
| y (yank) | Yes | Yes | Present |
| > (indent right) | Yes | Yes | Present |
| < (indent left) | Yes | Yes | Present |
| gq (format) | No | No | N/A |
| gu / gU (case) | No | No | N/A |
| ~ (toggle case) | No | No | N/A |

## 4. Text Objects

| Text Object | v0.8.1 | Current | Status |
|-------------|--------|---------|--------|
| iw, aw (word) | Yes | Yes | Present |
| iW, aW (WORD) | Yes | Yes | Present |
| i(, a( / i), a) | Yes | Yes | Present |
| i[, a[ / i], a] | Yes | Yes | Present |
| i{, a{ / i}, a} | Yes | Yes | Present |
| i<, a< / i>, a> | Yes | Yes | Present |
| i", a" | Yes | Yes | Present |
| i', a' | Yes | Yes | Present |
| i`, a` | Yes | Yes | Present |
| ip, ap (paragraph) | No | Yes | New |
| if, af (function) | Yes (treesitter) | Yes | Present (treesitter QueryCursor, #656) |
| ic, ac (class) | Yes (treesitter) | Yes | Present (treesitter QueryCursor, #656) |
| ia, aa (argument) | Yes (treesitter) | Yes | Present (treesitter QueryCursor, #656) |
| io, ao (conditional) | Yes (treesitter) | Yes | Present (treesitter QueryCursor, #656) |
| il, al (loop) | Yes (treesitter) | Yes | Present (treesitter QueryCursor, #656) |
| i/, a/ (comment) | Yes (treesitter) | Yes | Present (treesitter QueryCursor, #656) |
| iB, aB (block) | Yes (treesitter) | Yes | Present (commands exist, keybinding deferred, #656) |

## 5. Visual Mode

| Feature | v0.8.1 | Current | Status |
|---------|--------|---------|--------|
| v (character) | Yes | Yes | Present |
| V (line) | Yes | Yes | Present |
| Ctrl-V (block) | Yes | Yes | Present |
| o (swap anchor) | Yes | Yes | Present |
| gv (reselect) | No | Yes | New |
| d, y, c in visual | Yes | Yes | Present |
| >, < in visual | Yes | Yes | Present |

## 6. Search

| Feature | v0.8.1 | Current | Status |
|---------|--------|---------|--------|
| / forward search | Yes | Yes | Present |
| ? backward search | Yes | Yes | Present |
| n, N repeat search | Yes | Yes | Present |
| * word under cursor | Yes | Yes | Present |
| # reverse word search | Yes | Yes | Present |
| :noh clear highlight | Yes | Yes | Present |
| :s substitute | No | No | N/A |
| Search highlighting | Yes | Yes | Present |

## 7. Ex Commands

| Command | v0.8.1 | Current | Status |
|---------|--------|---------|--------|
| :w (write) | Yes | Yes | Present |
| :q (quit) | Yes | Yes | Present |
| :wq (write+quit) | Yes | Yes | Present |
| :e (edit file) | Yes | Yes | Present |
| :split / :sp | Yes | Yes | Present |
| :vsplit / :vs | Yes | Yes | Present |
| :close | Yes | Yes | Present |
| :only | Yes | Yes | Present |
| :set (options) | Yes | Yes | Present |
| :colorscheme | Yes | Yes | Present |
| :tabnew | Yes | Yes | Present |
| :tabclose | Yes | Yes | Present |
| :profile | Yes | Yes | Present |
| :buffers | No | Yes | New |
| :detach | No | Yes | New |
| :servers | No | Yes | New |
| :kill-server | No | Yes | New |
| :s (substitute) | No | No | Missing in both |

## 8. Window Management

| Feature | v0.8.1 | Current | Status |
|---------|--------|---------|--------|
| Horizontal split | Yes | Yes | Present |
| Vertical split | Yes | Yes | Present |
| Window navigation (Ctrl-W + hjkl) | Yes | Yes | Present |
| Window move (Ctrl-W + HJKL) | Yes | Yes | Present |
| Window resize (+, -, >, <) | Yes | Yes | Present |
| Equalize (Ctrl-W =) | Yes | Yes | Present |
| Close window (Ctrl-W c/q) | Yes | Yes | Present |
| Only window (Ctrl-W o) | Yes | Yes | Present |
| Float zone | No | Yes | New |
| Layer opacity | No | Yes | New |

## 9. Plugins/Modules

| Plugin (v0.8.1) | Module (current) | Status |
|------------------|-----------------|--------|
| Microscope (fuzzy finder) | microscope | Present |
| Explorer (file browser) | explorer | Present |
| Completion | completion | Present |
| LSP integration | lsp, lsp-navigation | Improved (split) |
| Treesitter (syntax) | treesitter-* (10 langs) | Improved (+2 langs) |
| Pair (brackets) | pair | Present |
| Which-Key | whichkey | Present |
| Cmdline completion | cmdline, completion | Present |
| Range-Finder | range-finder | Present |
| Statusline | statusline (driver trait) | Changed (trait-based) |
| Context | context | Present |
| Sticky Context | sticky-context | Present |
| Notification | notification | Present |
| Health Check | health-check | Present |
| Profiles | profiles | Present |
| Settings Menu | settings | Present |
| Pickers | picker-* (9 modules) | Improved (modular) |
| Landing Page | TUI ClientModule + Web Extension | Improved (breathing/roar animation, #657) |
| — | snippet | New |
| — | git-blame | New |
| — | git-signs | New |
| — | git-statusline | New |
| — | module-manager | New |
| — | indent-guide | New |
| — | emacs (personality) | New |
| — | tetromino (easter egg) | New |
| — | codec-* (7 codecs) | New |

## 10. Language Support

| Language | v0.8.1 | Current | Status |
|----------|--------|---------|--------|
| Rust | Yes | Yes | Present |
| Python | Yes | Yes | Present |
| JavaScript | Yes | Yes | Present |
| C | Yes | Yes | Present |
| JSON | Yes | Yes | Present |
| TOML | Yes | Yes | Present |
| Markdown | Yes | Yes | Present |
| Bash | Yes | Yes | Present |
| TypeScript | No | Yes | New |
| Go | No | Yes | New |

## 11. UI Features

| Feature | v0.8.1 | Current | Status |
|---------|--------|---------|--------|
| Statusline | Yes | Yes | Present |
| Command line | Yes | Yes | Present |
| Completion popup | Yes | Yes | Present |
| Which-key popup | Yes | Yes | Present |
| File explorer sidebar | Yes | Yes | Present |
| Notifications | Yes | Yes | Present |
| Line numbers | Yes | Yes | Present |
| Rainbow brackets | Yes | Yes | Present |
| Indent guides | No | Yes | New |
| Yank animation | Yes | Yes | Present (YankFlashBridge + inline_decorations, #657) |
| Git gutter signs | No | Yes | New |
| Git blame | No | Yes | New |

## Gap Summary

### Missing Features (from v0.8.1)

All previously identified gaps from v0.8.1 have been closed in Phase 14 (#653):

1. ~~**Semantic text objects**~~ — Closed by #656. Treesitter-based text objects
   for all 7 kinds (function, class, argument, conditional, loop, comment, block)
   with smallest-node selection for nested constructs.

2. ~~**Marks (user-facing)**~~ — Closed by #654. SetMark, GotoMarkLine,
   GotoMarkExact commands with full keybinding support.

3. ~~**Jump list (user-facing)**~~ — Closed by #654. Per-client Jumplist with
   push-on-jump for gg, G, n, N, *, #, mark goto. Ctrl-O/Ctrl-I keybindings.

4. ~~**Yank animation**~~ — Closed by #657. YankFlashBridge server bridge with
   TUI inline_decorations() and Web CSS highlight.

5. ~~**Animated landing mascot**~~ — Closed by #657. TUI breathing/roar color
   animation. Web CSS keyframe animations.

### Parity Assessment

**Overall parity: ~100%**

All v0.8.1 features have been reimplemented or superseded. The kernel rewrite
preserved all core editing functionality and added significant new features
(content codecs, multi-client sessions, FFI module loading, web client, CLI
client, semantic text objects, git integration, snippet expansion).

### New in Current (not in v0.8.1)

- Multi-client session model (tmux-like)
- gRPC v2 protocol with CLI and Web clients
- Content codecs (CJK, CSV, Hex, ELF, PDF)
- FFI dynamic module loading (.so plugins)
- Git blame and gutter signs
- Snippet expansion (TextMate/LSP format)
- Module manager UI
- Indent guides
- Emacs personality (proof-of-concept)
- W, B, E, ge, gE word motions
- f, F, t, T find-char motions with ;, , repeat
- Paragraph text objects (ip, ap)
- Float zone windows
- Display line motions (gj, gk)
- Visual reselect (gv)
- 2 additional languages (Go, TypeScript)
