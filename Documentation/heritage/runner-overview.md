# Runner overview

Historical orchestration model before the Phase 8 workspace split. The runner
combined server startup, clients, and module loading. Its successor paths are
listed in the [migration map](phase-8-migration.md).

## Responsibility

```text
Runner: startup, event dispatch, module loading, shutdown
    |
    +-- Server: RPC, sessions, viewports
    +-- Clients: CLI and TUI
    +-- Modules: editing policy
    +-- Drivers: services
    +-- Kernel: core mechanisms
```

The design moved calculations out of the event loop and into event handlers,
removing the large `AppStateRuntime` coordinator. The runner wired components
and dispatched work; modules and handlers supplied behavior.

## Event loop

```text
Read input -> emit KeyPressEvent -> process queued handlers
                                         |
                                         v
                              Render if requested -> repeat
```

Input reading could block; emission queued work, and `process_queue()` ran the
handlers. Rendering followed an explicit request. This distinction mattered
when testing whether key injection had completed its effects; see
[legacy synchronization lessons](legacy-memorial.md).
