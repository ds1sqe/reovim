# Layer Architecture

## The Core Rule

Each layer has ZERO policy that belongs to the layer above it.

```
   MECHANISM                                                         POLICY
  (What exist)                                                    (How decide)
       │                                                                │
       ▼                                                                ▼
╔═════════════════════════════════════════════════════════════════════════════╗
║                                                                             ║
║  ══════════════════════════════════════════════════════════════════════►    ║
║                                                                             ║
║  Kernel ──► Drivers ──► Runner ──► Modules ──► VimModule ──► UserConfig     ║
║                                                                             ║
║  Facts      Types      Storage    Capabilities   Behavior     Overrides     ║
║                                                                             ║
╚═════════════════════════════════════════════════════════════════════════════╝

  ◄────────── More General ──────────────────────────── More Specific ───────►
  ◄────────── More Stable ───────────────────────────── More Flexible ───────►
```

## Layer Responsibilities

```
╔═════════════════════════════════════════════════════════════════════════════╗
║                        USER CONFIGURATION                                   ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                                                                             ║
║  Responsibility: User-specific customizations                               ║
║  Examples: "<C-s>" = save, remap "jk" to Escape                             ║
║                                                                             ║
║  ┌────────────────────────────────────────────────────────────────────────┐ ║
║  │ ▸ Policy Budget: Personal preferences, workflow optimizations          │ ║
║  │ ▸ Zero Policy Of: Nothing above (top layer)                            │ ║
║  └────────────────────────────────────────────────────────────────────────┘ ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                     POLICY MODULE (modules/vim/)                            ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                                                                             ║
║  Responsibility: Define Vim-like editing behavior                           ║
║  Examples: hjkl movement, dd/yy operators, mode semantics                   ║
║                                                                             ║
║  ┌────────────────────────────────────────────────────────────────────────┐ ║
║  │ ▸ Policy Budget: "How Vim works" - keybindings, lookup behavior        │ ║
║  │ ▸ Zero Policy Of: User preferences                                     │ ║
║  └────────────────────────────────────────────────────────────────────────┘ ║
╠═════════════════════════════════════════════════════════════════════════════╣
║               MECHANISM MODULES (editor/, keymap/, motions/)                ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                                                                             ║
║  Responsibility: Provide editing CAPABILITIES                               ║
║  Examples: Mode stack, cursor movement, text operations                     ║
║                                                                             ║
║  ┌────────────────────────────────────────────────────────────────────────┐ ║
║  │ ▸ Policy Budget: "What operations exist" - command definitions         │ ║
║  │ ▸ Zero Policy Of: Vim behavior, keybindings, lookup preferences        │ ║
║  └────────────────────────────────────────────────────────────────────────┘ ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                              RUNNER                                         ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                                                                             ║
║  Responsibility: Store and retrieve data, orchestrate                       ║
║  Examples: KeymapRegistry stores bindings, Session manages state            ║
║                                                                             ║
║  ┌────────────────────────────────────────────────────────────────────────┐ ║
║  │ ▸ Policy Budget: "How to store/retrieve" - data structures             │ ║
║  │ ▸ Zero Policy Of: What bindings mean, how to interpret lookups         │ ║
║  └────────────────────────────────────────────────────────────────────────┘ ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                              DRIVERS                                        ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                                                                             ║
║  Responsibility: Define contracts, abstractions, types                      ║
║  Examples: KeyEvent, KeyLookupState, traits                                 ║
║                                                                             ║
║  ┌────────────────────────────────────────────────────────────────────────┐ ║
║  │ ▸ Policy Budget: "How to abstract" - type design, trait contracts      │ ║
║  │ ▸ Zero Policy Of: Storage impl, command behavior, keybindings          │ ║
║  └────────────────────────────────────────────────────────────────────────┘ ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                              KERNEL                                         ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                                                                             ║
║  Responsibility: Core primitives, pure facts                                ║
║  Examples: Buffer storage, event bus, memory management                     ║
║                                                                             ║
║  ┌────────────────────────────────────────────────────────────────────────┐ ║
║  │ ▸ Policy Budget: ZERO - pure mechanism only                            │ ║
║  │ ▸ Zero Policy Of: Everything above                                     │ ║
║  └────────────────────────────────────────────────────────────────────────┘ ║
╚═════════════════════════════════════════════════════════════════════════════╝
```

## Summary Table

```
┌────────────────────────┬───────────────────────────┬───────────────────────────────┐
│         Layer          │      Policy Budget        │       Zero Policy Of          │
├────────────────────────┼───────────────────────────┼───────────────────────────────┤
│  Kernel                │  ZERO                     │  Everything                   │
│  Drivers               │  Type/trait design        │  Storage, behavior            │
│  Runner                │  Storage/retrieval        │  Interpretation               │
│  Mechanism Modules     │  Capabilities             │  Vim behavior, keybindings    │
│  Policy Module (vim/)  │  Vim behavior             │  User preferences             │
│  User Config           │  Personal prefs           │  (top layer)                  │
└────────────────────────┴───────────────────────────┴───────────────────────────────┘
```

## Direction of Dependencies

```
            ┌─────────────────────────────────────────────────┐
            │                                                 │
            │     Layer N provides MECHANISMS                 │
            │     Layer N+1 provides POLICY                   │
            │     using those mechanisms                      │
            │                                                 │
            │     Layer N has ZERO knowledge                  │
            │     of Layer N+1's decisions                    │
            │                                                 │
            └─────────────────────────────────────────────────┘
                                    │
                                    ▼
            ┌─────────────────────────────────────────────────┐
            │                                                 │
            │     Higher layers DEPEND ON lower layers.       │
            │     Lower layers NEVER depend on higher.        │
            │     Lower layers NEVER make decisions           │
            │     for higher layers.                          │
            │                                                 │
            └─────────────────────────────────────────────────┘
```

## Test for Correctness

For any piece of code, ask:

> "Could someone build a completely different editor paradigm using this layer?"

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  If YES  ──►  Layer is pure mechanism  ✓                                    │
│  If NO   ──►  Layer has leaked policy from above  ✗                         │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Example: Could Emacs use KeymapRegistry.query()?                           │
│                                                                             │
│  • Old lookup()  ──►  NO  - forced Vim's "wait for longer" behavior         │
│  • New query()   ──►  YES - just returns facts                              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## See Also

- [Mechanism vs Policy Philosophy](../../contributing/philosophy/mechanism-vs-policy.md) - Unix philosophy origins
- [Violations Guide](violations.md) - What NOT to do
- [Vision](vision.md) - The Policy-Composable Editor promise
