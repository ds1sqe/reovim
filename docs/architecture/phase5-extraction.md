# Phase 5: Concept-Extraction Strategy

## NOT Migration - Concept-Extraction

Phase 5 is **NOT** a migration. It's a **concept-extraction** and **fresh implementation**.

| Term | Meaning | Used In Phase 5? |
|------|---------|------------------|
| **Migration** | Move existing code from A to B | ❌ NO |
| **Move** | Copy code to new location | ❌ NO |
| **Concept-Extraction** | Study A, understand concepts, implement fresh in B | ✅ YES |

## OLD vs NEW

### OLD (Reference Only - Will Be Deleted)

```
lib/core/                    ← LEGACY v0.8.x
├── src/
│   ├── runtime/             ← Reference for lib/kernel/sched/
│   ├── buffer/              ← Reference for lib/kernel/mm/
│   ├── event_bus/           ← Reference for lib/kernel/ipc/
│   ├── screen/              ← Reference for lib/drivers/display/
│   ├── frame/               ← Reference for lib/drivers/display/
│   ├── compositor/          ← Reference for lib/drivers/display/
│   ├── option/              ← Reference for lib/kernel/core/
│   ├── config/              ← Reference for lib/drivers/*/
│   └── ...
└── Cargo.toml

Status: EXISTS NOW, DELETE AFTER PHASE 5
```

### NEW (Fresh Implementation)

```
lib/kernel/                  ← NEW v0.9.0+
├── mm/                      ← Memory management (buffers, cache)
├── ipc/                     ← Inter-process communication (events)
├── core/                    ← Core types (modes, options)
├── block/                   ← Block devices (files)
├── sched/                   ← Scheduler (runtime, tasks)
└── api/                     ← Public API boundary

lib/drivers/                 ← NEW v0.9.0+
├── syntax/                  ← Syntax highlighting driver
├── input/                   ← Input handling driver
├── display/                 ← Display/rendering driver
├── lsp/                     ← LSP client driver
├── net/                     ← Network driver
├── vfs/                     ← Virtual filesystem driver
└── log/                     ← Logging driver

modules/                     ← NEW v0.9.0+ (Policy)
├── keymap/                  ← Keybinding policy
├── motions/                 ← Motion policy
├── operators/               ← Operator policy
├── layout/                  ← Window layout policy
└── options/                 ← Default options policy

Status: IMPLEMENT FRESH, INFORMED BY lib/core/
```

## The Process

```
┌─────────────────────────────────────────────────────────────────────────┐
│  PHASE 5 WORKFLOW                                                       │
│                                                                         │
│  1. STUDY lib/core/X          "What concepts does this implement?"      │
│            │                                                            │
│            ▼                                                            │
│  2. DESIGN new API            "How should this work in kernel model?"   │
│            │                                                            │
│            ▼                                                            │
│  3. IMPLEMENT in lib/kernel/  "Fresh code, clean architecture"          │
│     or lib/drivers/                                                     │
│     or modules/                                                         │
│            │                                                            │
│            ▼                                                            │
│  4. TEST new implementation   "Does it work correctly?"                 │
│            │                                                            │
│            ▼                                                            │
│  5. DELETE lib/core/X         "Remove legacy after new works"           │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Why Not Migration?

1. **Clean Architecture**: lib/core/ has accumulated technical debt. Fresh implementation allows proper layering.

2. **Mechanism vs Policy**: lib/core/ mixes mechanism and policy. Extraction lets us separate them correctly.

3. **API Boundary**: lib/core/ has no clear API boundary. New kernel enforces `pub(crate)` internally, only `api/` is public.

4. **No Backward Compatibility Hacks**: Migration often preserves bad patterns for compatibility. Extraction doesn't.

## Terminology Guide

### In Issue Files

| Don't Say | Say Instead |
|-----------|-------------|
| "Migrate X from lib/core" | "Implement X in lib/kernel" |
| "Move code to" | "Implement fresh in" |
| "Files to migrate" | "Reference: lib/core/, Implement: lib/kernel/" |
| "After migration" | "After Phase 5 complete" |
| "Migrated successfully" | "Implemented and verified" |

### File Section Headers

```markdown
## Reference (OLD - lib/core/)
[What to study, what concepts exist]

## Implementation (NEW - lib/kernel/)
[What to create fresh]

## Cleanup (After Phase 5)
[What to delete from lib/core/]
```

## Coexistence During Phase 5

During Phase 5, both OLD and NEW exist simultaneously:

```
reovim/
├── lib/core/          ← OLD: Still runs, being phased out
├── lib/kernel/        ← NEW: Being implemented
├── lib/drivers/       ← NEW: Being implemented
└── modules/           ← NEW: Being implemented
```

The editor continues to work using lib/core/ while lib/kernel/ is being built. Once a subsystem is fully implemented and tested in the NEW architecture, the corresponding OLD code is deleted.

## Summary

- **lib/core/** = OLD, reference only, will be deleted
- **lib/kernel/** = NEW, fresh implementation
- **lib/drivers/** = NEW, fresh implementation
- **modules/** = NEW, fresh implementation
- **Process** = Study OLD → Design NEW → Implement NEW → Test → Delete OLD
