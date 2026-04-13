# Keymap Implementation

This document describes the keymap system's mechanism/policy separation as implemented in Epic #353.

## Two Concerns

The keymap system handles two distinct concerns:

```
┌─────────────────┬────────────────────────────────┬────────────────────────┬───────────┐
│     Concern     │           Question             │     How it works       │ Composes? │
├─────────────────┼────────────────────────────────┼────────────────────────┼───────────┤
│  Bindings       │  Which command for this key?   │  Layered lookup        │  Yes      │
│                 │                                │  User > Policy > Base  │           │
├─────────────────┼────────────────────────────────┼────────────────────────┼───────────┤
│  Behavior       │  Wait for `dd` or execute `d`? │  ModeKeyResolver       │  No       │
│                 │                                │  (one per mode)        │           │
└─────────────────┴────────────────────────────────┴────────────────────────┴───────────┘
```

## Architecture

```
╔═══════════════════════════════════════════════════════════════════════════╗
║  USER CONFIG (~/.config/reovim/)                            OVERRIDES     ║
║                                                                           ║
║  ▸ Binding overrides (top layer in registry)                              ║
║  ▸ Resolver selection (swap Vim for Eager per mode)                       ║
╠═══════════════════════════════════════════════════════════════════════════╣
║  POLICY MODULE (modules/vim/)                               DEFAULTS      ║
║                                                                           ║
║  ▸ ModeKeyResolver implementations (VimNormalResolver)                    ║
║  ▸ Default bindings (middle layer in registry)                            ║
╠═══════════════════════════════════════════════════════════════════════════╣
║  MECHANISM (server + drivers)                                ENGINE       ║
║                                                                           ║
║  ▸ KeymapRegistry.query() returns FACTS                                   ║
║  ▸ Layered binding lookup                                                 ║
║  ▸ ModeKeyResolver trait (interface only)                                 ║
╚═══════════════════════════════════════════════════════════════════════════╝
```

## KeyLookupState

The registry's `query()` method returns pure facts:

```rust
/// Pure mechanism: reports what exists in the registry.
/// This enum reports FACTS. It does NOT make decisions.
pub enum KeyLookupState {
    /// Exact match exists, no longer bindings
    ExactOnly(CommandId),

    /// Exact match exists AND longer bindings exist
    /// e.g., 'd' matches AND 'dd' exists
    ExactWithLonger { exact: CommandId },

    /// No exact match, but longer bindings exist
    /// e.g., 'g' has no binding but 'gg' exists
    PrefixOnly,

    /// Nothing matches
    NotFound,
}
```

## Data Flow

```
                           User presses 'd'
                                  │
                                  ▼
╔════════════════════════════════════════════════════════════════════════════════╗
║                                                                                ║
║  Server: KeymapRegistry.query("d")                                             ║
║  ──────────────────────────────────                                            ║
║                                                                                ║
║  exact_match = Some("enter-delete-operator")                                   ║
║  has_longer  = true  (because "dd", "dw" exist)                                ║
║                                                                                ║
║  ┌──────────────────────────────────────────────────────────────────────────┐  ║
║  │ Returns: KeyLookupState::ExactWithLonger {                               │  ║
║  │              exact: "enter-delete-operator"                              │  ║
║  │          }                                                               │  ║
║  └──────────────────────────────────────────────────────────────────────────┘  ║
║                                                                                ║
║  ◄── PURE FACT: "Here's what exists, you decide"                               ║
║                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════╝
                                  │
                                  ▼
╔════════════════════════════════════════════════════════════════════════════════╗
║                                                                                ║
║  modules/vim/: VimLookupPolicy.resolve(state)                                  ║
║  ─────────────────────────────────────────────                                 ║
║                                                                                ║
║  match state {                                                                 ║
║      ExactWithLonger { .. } => Prefix,  ◄── Vim decides: wait                  ║
║      ...                                                                       ║
║  }                                                                             ║
║                                                                                ║
║  ┌──────────────────────────────────────────────────────────────────────────┐  ║
║  │ Returns: KeyLookupResult::Prefix                                         │  ║
║  └──────────────────────────────────────────────────────────────────────────┘  ║
║                                                                                ║
║  ◄── POLICY: "Vim says wait for more keys"                                     ║
║                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════╝
                                  │
                                  ▼
                   EventLoop waits for more keys...
```

## Layered Bindings

Bindings compose through layers:

```rust
pub enum BindingLayer {
    /// Base layer (mechanism defaults, rarely used)
    Base = 0,
    /// Policy module layer (Vim bindings, Emacs bindings, etc.)
    Policy = 1,
    /// User configuration layer (highest priority)
    User = 2,
}
```

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           LAYER PRIORITY                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│      ┌─────────────────┐                                                    │
│  2   │   User Layer    │  ◄── Highest priority (user overrides)             │
│      └────────┬────────┘                                                    │
│               │ shadows                                                     │
│      ┌────────▼────────┐                                                    │
│  1   │  Policy Layer   │  ◄── Module defaults (Vim bindings)                │
│      └────────┬────────┘                                                    │
│               │ shadows                                                     │
│      ┌────────▼────────┐                                                    │
│  0   │   Base Layer    │  ◄── Mechanism defaults (rarely used)              │
│      └─────────────────┘                                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

Lookup returns the binding from the highest layer:

```rust
// Policy layer: d -> delete
registry.register_at_layer(BindingLayer::Policy, &normal, keys("d"), cmd("delete"));

// User layer: d -> custom_delete (overrides policy)
registry.register_at_layer(BindingLayer::User, &normal, keys("d"), cmd("custom_delete"));

// User layer wins
assert_eq!(registry.get_binding(&normal, &keys("d")), Some(cmd("custom_delete")));
```

## Alternative Policies

The same facts can produce different behaviors with different resolvers:

```
┌─────────────────────────┬─────────────────────────────────────────────────────┐
│        Policy           │         ExactWithLonger behavior                    │
├─────────────────────────┼─────────────────────────────────────────────────────┤
│  VimLookupPolicy        │  Wait for dd, dw, etc.                              │
│  EagerLookupPolicy      │  Execute d immediately                              │
│  TimeoutLookupPolicy    │  Wait 500ms, then execute exact                     │
│  KakouneLookupPolicy    │  Select first, then operate                         │
└─────────────────────────┴─────────────────────────────────────────────────────┘

           All possible because Server returns FACTS, not DECISIONS.
```

## What We Achieved (Epic #353)

```
┌───────────────────────────────────┬───────────────────────────────────────────┐
│             BEFORE                │                AFTER                      │
├───────────────────────────────────┼───────────────────────────────────────────┤
│                                   │                                           │
│  • Server's lookup() made         │  • KeymapRegistry.query() returns         │
│    Vim-specific decisions         │    pure facts                             │
│                                   │                                           │
│  • Vim keybindings in             │  • ModeKeyResolver applies policy         │
│    modules/keymap/                │                                           │
│                                   │  • Vim bindings in modules/vim/           │
│  • No separation between          │                                           │
│    facts and interpretation       │  • User overrides via keymap.toml         │
│                                   │                                           │
└───────────────────────────────────┴───────────────────────────────────────────┘
```

## User Configuration

Users can override bindings and select resolvers:

```toml
# ~/.config/reovim/keymap.toml

# Add/override bindings (User layer - highest priority)
[bindings.normal]
"<C-s>" = "buffer:save"
"<C-q>" = "app:quit"

[bindings.insert]
"jk" = "mode:normal"   # Quick escape

# Remove bindings
[remove.normal]
keys = ["Q"]           # Disable Q in normal mode

# Swap resolver for a mode (advanced)
[resolvers]
normal = "vim"         # default
insert = "vim"         # default
game = "eager"         # for game modes
```

## See Also

- [User Keymap Configuration](../../user-guide/keymap-config.md) - End-user guide
- [Violations Guide](violations.md) - Anti-patterns to avoid
- [Mechanism vs Policy Philosophy](../../contributing/philosophy/mechanism-vs-policy.md) - Unix origins
