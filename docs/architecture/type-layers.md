# Type Layers in Reovim

This document clarifies which types belong to which layer and how they interact.

## Overview

Reovim follows a **Linux kernel-inspired architecture** with clear separation between layers.
Understanding which types belong where prevents layer confusion and import conflicts.

```text
┌─────────────────────────────────────────────────────────────────────────┐
│  MODULES (server/modules/)                                  POLICY      │
│  ExCommandContext, ExCommandHandler, VimMode, Operator, ...             │
│  → Decides HOW things behave                                            │
├─────────────────────────────────────────────────────────────────────────┤
│  DRIVERS (ext/server/drivers/)                             MECHANISM    │
│  CommandContext, CommandHandler, KeySequence, ResolveContext, ...       │
│  → Provides services, defines trait contracts                           │
├─────────────────────────────────────────────────────────────────────────┤
│  KERNEL (server/lib/kernel/)                               MECHANISM    │
│  BufferId (mm/), WindowId (mm/), TabId (mm/)                            │
│  ModeId (core/), CommandId (core/)                                      │
│  ByteEdit (block/), KernelContext, ...                                  │
│  → Core primitives, WHAT can be done                                    │
│  → NOTE: Position/TextPosition/TextEdit moved to DOMAIN layer (#739/#740)│
├─────────────────────────────────────────────────────────────────────────┤
│  DOMAIN (ext/server/domain/text/) — reovim-domain-text         SHARED      │
│  Position, TextPosition, TextEdit, Motion, TextObject                   │
│  Domain trait (ext/server/domain/domain/) — reovim-domain                         │
│  → Shared text-editing domain types, usable by kernel, drivers, modules │
├─────────────────────────────────────────────────────────────────────────┤
│  SERVER (server/lib/server/)                               MECHANISM    │
│  EventLoop, ModeRegistry, CommandRegistry, SessionState, ...            │
│  → Runtime orchestration, wires everything together                     │
└─────────────────────────────────────────────────────────────────────────┘
```

## Context Types

Context types are NOT duplicates - they serve different lifecycle stages:

| Type | Layer | Purpose | Lifecycle |
|------|-------|---------|-----------|
| `KernelContext` | kernel | Access to kernel services | Long-lived, passed around |
| `ModuleContext` | kernel | Module initialization | Init-time only |
| `SessionRuntime` | session driver | Borrows Session + per-client state | Per-command |
| `ResolveInput` | input driver | Key resolution input | Per-key |
| `ResolveContext` | input driver | Command execution args | Per-command |
| `TransitionContext` | input driver | Mode transition input | Per-transition |
| `CommandContext` | command driver | Command execution context | Per-command |
| `ExCommandContext` | commands module | Ex-command specific context | Per-ex-command |
| `RpcContext` | server | gRPC handling context | Per-RPC call |

### Why So Many Contexts?

Each context type has a specific **scope** and **lifetime**:

```text
Session starts
    └─> SessionRuntime created (borrows Session + per-client EditingState)
        │
        └─> Key pressed
            └─> ResolveInput passed to resolver
                │
                └─> ResolveContext returned with command args
                    │
                    └─> CommandContext created for execution
                        │
                        └─> Command executed
```

Combining these into a "god context" would:
1. Violate separation of concerns
2. Create tight coupling between layers
3. Make testing harder

## Command Types

There are two distinct command systems:

### Driver Command (lib/drivers/command/)

General-purpose command trait for any module:

```rust
// Mechanism - defines the contract
pub trait CommandHandler: Send + Sync {
    fn id(&self) -> CommandId;
    fn execute(&self, ctx: &CommandContext) -> CommandResult;
    // ...
}
```

### Ex-Command (modules/commands/)

Vim-style colon commands (`:w`, `:q`, etc.):

```rust
// Policy - implements the contract for ex-commands
pub trait ExCommandHandler: Send + Sync {
    fn id(&self) -> &'static str;
    fn names(&self) -> &[&'static str];  // e.g., ["w", "write"]
    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str]) -> Result<(), CommandError>;
    // ...
}
```

**Why the distinction?**
- `CommandHandler` is generic for any command type
- `ExCommandHandler` is specific to vim ex-command semantics (names, ranges, bang)

## Mode System

### Mode Identity (Kernel)

```rust
// Kernel provides identity only
pub trait Mode: Send + Sync + 'static {
    fn module() -> ModuleId;
    fn id(&self) -> ModeId;
    fn display_name(&self) -> &'static str;
    fn cursor_style(&self) -> CursorStyle;
    fn accepts_char_input(&self) -> bool;  // Behavior
}
```

### Mode Display (Display Driver)

```rust
// Display driver provides rendering traits
pub trait ModeDisplay: Send + Sync {
    fn cursor_style(&self) -> CursorStyle;
    fn status_text(&self) -> &'static str;
}
```

### Mode Resolution (Input Driver)

```rust
// Input driver provides key resolution contract
pub trait ModeKeyResolver: Send + Sync {
    fn resolve(&self, key: &KeyEvent, state: &mut ModeState) -> ResolveResult;
    fn mode_id(&self) -> &ModeId;
}
```

### ModeRegistry (Server)

```rust
// Server manages registered modes at runtime
pub struct ModeRegistry {
    // Maps ModeId -> mode properties (cursor style, display name, etc.)
}
```

**Note:** The input driver previously had a `ModeRegistry` type that was unused.
It was removed in #391 - the server's `ModeRegistry` is the canonical implementation.

## Resolver Types

The resolver system in the input driver has several types for the key resolution flow:

| Type | Purpose | Stage |
|------|---------|-------|
| `ResolveInput` | Keys + mode + keymap access | Input |
| `ModeState` | Mode stack + pending keys + buffer | During |
| `ResolveResult` | Execute/Pending/ModeTransition | Output |
| `ResolveContext` | Count + register + metadata | → Command |
| `TransitionContext` | Operator + count + register | → Enter mode |
| `PopResult` | OperatorRange/Cancelled/etc. | → Exit mode |
| `ModeTransition` | Push/Pop/Set mode | Control flow |
| `OperatorArgs` | Shared count/register fields | Utility |

### Data Flow Example (Vim `dw`)

```text
Normal mode: d pressed
    → TransitionContext { pending_operator: "delete", count: 3, register: 'a' }
    → ModeTransition::Push { mode: operator-pending, context }

Operator-pending mode: w pressed
    → PopResult::OperatorRange {
        operator: "delete",  ← from TransitionContext
        count: 3,            ← from TransitionContext
        register: 'a',       ← from TransitionContext
        start, end, linewise ← computed by motion
      }
    → ModeTransition::Pop { result }

Runner: executes delete command with all info from PopResult
```

## Naming Conventions

To avoid name collisions between layers:

| Pattern | Example | When to Use |
|---------|---------|-------------|
| `Ex*` prefix | `ExCommandContext`, `ExCommandHandler` | Vim ex-command specific types |
| Layer suffix | `RpcContext`, `ResolveContext` | Contexts specific to a layer |
| Descriptive | `OperatorArgs`, `TransitionContext` | Self-documenting purpose |

### Historical Collisions (Fixed in #391)

| Collision | Resolution |
|-----------|------------|
| `CommandContext` (driver) vs `CommandContext` (module) | Module renamed to `ExCommandContext` |
| `CommandHandler` (driver) vs `CommandHandler` (module) | Module renamed to `ExCommandHandler` |
| `PopResult` (session) vs `PopResult` (input) | Session driver's was dead code, deleted |
| `ModeRegistry` (input) vs `ModeRegistry` (runner) | Input driver's was dead code, deleted |

## Deleted Types (Dead Code)

The following types were removed in #391 as unused:

| Type | Former Location | Reason |
|------|-----------------|--------|
| `ModeInput` | input driver | Redundant with kernel's `Mode::accepts_char_input()` |
| `PopResult` | session driver | Only used in tests, superseded by input driver's version |
| `ModeRegistry` | input driver | Runner's implementation is used instead |
| `ModeEntry` | input driver | Part of the unused ModeRegistry |

## Summary

1. **Context types are intentionally different** - they serve different lifecycle stages
2. **The mode stack is correct architecture** - needed for operator-pending mode
3. **Name collisions should use prefixes** - `Ex*` for ex-commands, layer suffixes for contexts
4. **Dead code should be deleted** - unused types cause confusion
5. **Types should document their layer** - doc comments should explain which layer owns them
