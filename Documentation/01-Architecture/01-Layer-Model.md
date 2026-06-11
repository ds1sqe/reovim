# 1.1 — Layer Model

**Scope.** The horizontal tier diagram of the system: where mechanism
ends, where policy begins, what crosses each boundary.

**Locked rules.** None directly; this chapter constrains rule
placement in `01-Architecture/02-Project-Layout-and-DAG.md` (`DAG*`)
and `08-Client/01-Layer-Model.md` (`CL*`).

---

## 1. Tier diagram

```
┌─────────────────────────────────────────────────────────────────┐
│ COMPOSITION   apps/server, apps/tui, apps/cli, apps/web,        │
│               apps/reovim (launcher; embedded + subprocess)     │
├─────────────────────────────────────────────────────────────────┤
│ EXT (server)  ext/server/{modules, drivers, providers, domain}  │
│ EXT (client)  ext/client/{platforms, driver, module,            │
│                          capabilities}    (flat-category tree)  │
├─────────────────────────────────────────────────────────────────┤
│ RUNTIME       server/lib/server/* (framed protocol, dispatch glue) │
├─────────────────────────────────────────────────────────────────┤
│ KERNEL        server/lib/kernel/* (Kernel, registries, scheduler) │
├─────────────────────────────────────────────────────────────────┤
│ CONTRACTS     server/lib/subsys/* (closed server contracts)     │
│               clients/lib/subsys/* (closed client contracts)    │
├─────────────────────────────────────────────────────────────────┤
│ FOUNDATION    arch/, lib/*, uapi/*                              │
└─────────────────────────────────────────────────────────────────┘
```

## 2. Boundary contracts

| Boundary | Surface | Crossing rule |
|---|---|---|
| Foundation → Contracts | Rust types, traits | foundation never depends on contracts. |
| Contracts → Kernel | Rust types, traits | kernel never reaches into ext. |
| Kernel → Runtime | Rust types | runtime composes kernel; no upward dep. |
| Kernel/Runtime ↔ Ext (cdylib) | `#[repr(C)]` ABI, opaque handles, primitive bytes | no Rust trait object crosses; see 6.1. |
| Apps → Sibling apps | only via launcher's `embedded-*` features (see 1.3) | non-launcher apps may not link sibling apps. |
| Runtime → Tools | forbidden | tools may compose tiers for testing only. |

## 3. Mechanism vs policy

| Concern | Mechanism (kernel) | Policy (ext) |
|---|---|---|
| Buffer storage | mm/, scheduler, dispatch, IPC | which Domain mounts where |
| Input | `RawInput { kind, payload }` framing | keymaps, modes, motions, registers, leaders |
| Coordination | carrier framing + codec registry | per-Domain content semantics |
| Config | layer stack + schema validation | per-participant fields, defaults, trust |
| Streams | substrate (S1..S10) | concrete schemes (PTY, file watcher, process) |
| Render | RenderTarget contract | concrete renderers (TUI cells, web SVG) |

## 4. What this chapter forbids

- Kernel may not import any ext crate.
- Contracts may not import any ext crate.
- Apps (other than launcher) may not link sibling apps.
- Tools may not be linked from any shipping crate.

## 5. What this chapter delegates

- Exact crate-edge listing → `01-Architecture/02-Project-Layout-and-DAG.md`.
- Composition-root scope → `01-Architecture/03-Apps-and-Invocation.md`.
- Client tier internals → `08-Client/01-Layer-Model.md`.
- ABI surface details → `06-ABI/01-Surface.md`.

## Open items

None — this chapter is a synopsis; its rules live downstream.

## Conformance

This chapter has no rules. Subordinate chapters carry `DAG*`, `AL*`,
`CL*` rules whose conformance fixtures live in
`09-Conformance/01-Rule-Matrix.md`.
