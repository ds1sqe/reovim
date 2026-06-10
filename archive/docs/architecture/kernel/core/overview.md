# core/ - Core Primitives

Mode identity, editor configuration, and option registry.

## Source Location

`server/lib/kernel/src/core/`

## Overview

Following the domain decoupling in #740, text-specific algorithms (`Motion`,
`TextObject`, `Register`, `Jumplist`) were moved to `reovim-domain-text`, and
mark types (`Mark`, `MarkBank`, `SpecialMark`) were moved to
`reovim-driver-session`.

The `core/` subsystem now holds three closely related kernel-owned abstractions:

- **Mode** — runtime mode identity and the `Mode` trait for policy modules
- **Config** — thread-safe flat key-value configuration store
- **Option** — scoped option registry for editor settings

All are mechanism only. Policy (which modes exist, what options mean) belongs
in drivers and modules.

## Mode System

### `ModeId`

Namespaced mode identifier combining a `ModuleId`, a display name, and a numeric
`discriminant`. Equality is based on module + discriminant only (the name is
display-only).

```rust
use reovim_kernel::api::v1::{ModeId, ModuleId};

let module = ModuleId::new("vim");
let normal = ModeId::with_discriminant(module.clone(), "NORMAL", 0);
let insert = ModeId::with_discriminant(module, "INSERT", 1);

assert_ne!(normal, insert);
assert_eq!(normal.discriminant(), 0);
```

### `Mode` trait

Policy modules implement this trait on their mode enums. The trait is not
object-safe by design — runtime storage uses `ModeId`.

```rust
pub trait Mode: Copy + Clone + PartialEq + Eq + Hash + Send + Sync + 'static {
    fn module() -> ModuleId where Self: Sized;
    fn discriminant(&self) -> u16;
    fn display_name(&self) -> &'static str;
    fn cursor_style(&self) -> CursorStyle;
    fn accepts_char_input(&self) -> bool;
    // optional: has_selection, inherits_from, is_entry
}
```

A blanket `From<M> for ModeId` is provided for all `Mode` types.

### `ModeStack`

Push/pop stack for vim-style mode nesting. Always has at least one element.
Accepts `impl Into<ModeId>` on all mutating methods.

```rust
let mut stack = ModeStack::new(normal.clone());
stack.push(op_pending.clone());
assert_eq!(stack.current(), &op_pending);
stack.pop();
assert_eq!(stack.current(), &normal);
```

### `CursorStyle`

```rust
pub enum CursorStyle { Block, Bar, Underline, Hidden }
```

### `CommandId`

Namespaced command identifier. `CommandId::new` is `const fn` (zero allocation).
`CommandId::from_qualified("module:name")` handles dynamic runtime identifiers
via `Cow<'static, str>` — no memory leak.

## Configuration

### `Config`

Thread-safe flat key-value store. Keys use dot notation (`editor.theme`,
`plugin.lsp.timeout`). Backed by a `RwLock<HashMap<String, ConfigValue>>`.

```rust
use reovim_kernel::api::v1::{Config, ConfigValue};

let config = Config::new();
config.set_str("editor.theme", "dark");
config.set_int("editor.tabwidth", 4);
config.set_bool("editor.number", true);

assert_eq!(config.get_str("editor.theme"), Some("dark".to_string()));
```

The kernel does not parse TOML. TOML parsing is provided by modules (no serde
dependency in the kernel).

### `ConfigValue`

```rust
pub enum ConfigValue {
    Bool(bool),
    Integer(i64),
    String(String),
    Array(Vec<Self>),
    Table(HashMap<String, Self>),
}
```

### `ConfigPaths`

XDG-compliant path resolution for config, data, and cache directories.
All paths can be overridden via `REOVIM_CONFIG_DIR`, `REOVIM_DATA_DIR`, and
`REOVIM_CACHE_DIR` environment variables (used for worktree isolation in tests).

## Option Registry

`OptionRegistry` provides scope-aware storage for editor settings. Modules
register `OptionSpec` definitions during initialisation; the registry handles
storage and scope fallback.

### Scope resolution order

1. Window-local value (scope `Window`)
2. Buffer-local value (scope `Buffer` or `Window`)
3. Global override
4. Default from spec

### Types

| Type | Purpose |
|------|---------|
| `OptionRegistry` | Centralized option storage with scope fallback |
| `OptionSpec` | Spec for one option: name, type, default, short alias |
| `OptionValue` | Type-safe option value (Bool, Int, String, ...) |
| `OptionScope` | `Global`, `Buffer`, or `Window` scope |
| `OptionScopeId` | Identifies the scope instance (global / buffer id / window id) |
| `OptionConstraint` | Validation rule applied on set |
| `OptionError` / `SetResult` | Error types |

## API Exports

```rust
use reovim_kernel::api::v1::{
    // Mode system
    Mode, ModeId, ModeStack, CursorStyle, CommandId,
    // Config
    Config, ConfigValue, ConfigPaths, ConfigError,
    // Options
    OptionRegistry, OptionSpec, OptionValue, OptionScope, OptionScopeId,
    OptionConstraint, ConstraintError, OptionError, SetResult,
};
```

## Moved Types

The following types were removed from `core/` as part of #740:

| Type | New location |
|------|-------------|
| `Motion`, `TextObject`, `MotionEngine` | `reovim-domain-text` |
| `Register`, `RegisterBank` | `reovim-domain-text` |
| `Jumplist` | `reovim-domain-text` |
| `Mark`, `MarkBank`, `SpecialMark` | `reovim-driver-session` |

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [mm Subsystem](../mm/overview.md) - Buffer and window identifiers
- [Module-Mode Inheritance](../../modules/mode-inheritance.md) - Mode system usage
