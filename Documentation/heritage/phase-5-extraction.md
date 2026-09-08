# Phase 5: concept extraction

Historical strategy for replacing the v0.8 core during Epic #150. Phase 5
studied existing behavior and implemented it afresh under the v0.9 boundaries.
The [phase history](project-kernel-phases.md) records the results.

## Why rebuild

`lib/core/` mixed mechanisms, feature policy, and presentation without a clear
public boundary. Moving its files would preserve that coupling. The new
implementation kept kernel internals `pub(crate)` and exposed `api/` to
consumers.

## Reference map

These are historical donor and destination paths, not today's source tree.

| Donor under `lib/core/src/` | Destination | Concept |
| --- | --- | --- |
| `runtime/` | `lib/kernel/sched/` | Runtime and scheduled work. |
| `buffer/` | `lib/kernel/mm/` | Text storage and caches. |
| `event_bus/` | `lib/kernel/ipc/` | Events and subscriptions. |
| `option/` | `lib/kernel/core/` | Typed options. |
| `screen/`, `frame/`, `compositor/` | `lib/drivers/display/` | Rendering and windows. |
| `config/` | `lib/drivers/*/` | Service-specific configuration. |
| `keystroke/` | `lib/drivers/input/` | Input parsing. |
| `highlight/` | `lib/drivers/syntax/` | Syntax integration. |
| `command/` | `lib/drivers/command/` | Command dispatch. |
| `plugin/` | `modules/` | Feature policy and lifecycle. |

Keymap, motions, operators, layout, and default options became policy modules.
Drivers supplied display, input, syntax, LSP, network, VFS, and logging
services.

## Workflow

```text
Study donor behavior
    |
    v
Design the new public boundary
    |
    v
Implement -> test -> retire the replaced legacy path
```

Issue notes separated the donor reference, new implementation, and cleanup.
Both implementations coexisted while the old core still ran the editor;
retirement followed verification of each replacement. #215 archived the
legacy tree. See [legacy lessons](legacy-memorial.md) for what it taught us.
