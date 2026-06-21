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
│ EXT (server)  editor/{modules, drivers, providers, domains}     │
│ EXT (client)  client/{platforms, drivers, modules,              │
│                       capabilities}       (flat-category tree)  │
├─────────────────────────────────────────────────────────────────┤
│ RUNTIME       editor/lib/server/* (framed protocol, dispatch glue) │
├─────────────────────────────────────────────────────────────────┤
│ KERNEL        editor/lib/kernel/* (Kernel, registries, scheduler) │
├─────────────────────────────────────────────────────────────────┤
│ CONTRACTS     editor/lib/subsys/* (closed server contracts)     │
│               client/lib/subsys/* (closed client contracts)     │
├─────────────────────────────────────────────────────────────────┤
│ FOUNDATION    uapi/* · kabi/* · lib/* · platform-* · arch-*     │
└─────────────────────────────────────────────────────────────────┘
```

**Kernel naming note.** The `KERNEL` tier in this diagram is the
**editor/server kernel** (`editor/lib/kernel/*`): the Math-layer mechanism
for sessions, registries, scheduling, and editor state. It is not the
World-layer **system kernel**. The system-kernel crate lives in Foundation as
`system/lib/kernel`: it is the bridge from the product-facing `uapi/*` face
to the machine-facing `kabi/*` face, carrying common World services such as
boot assembly, console/splash policy, device inventory shaping, and FDT
policy over caller-supplied facts and surfaces. It is not hardware-specific
and may not import `arch-*`, `arch-sys-*`, `arch-floor-*`, or `platform-*`.

### 1.1 Foundation stack (up-face / down-face)

The FOUNDATION tier is itself layered. It is not a flat bag of crates; it has
two semantic faces and one bridge. The product-facing side imports `uapi/*`.
Hardware/chip/device-specific code imports `kabi/*`. `system/lib/kernel` is
the bridge that may name both.

```
        ▲ up-face — product/over-layers import `uapi/*`
        │
  uapi/posix      canonical product-visible POSIX values
  uapi/abi        module/plugin cdylib ABI surface
  uapi/protocol   framed client↔server protocol surface
  ─────────────── contract line ───────────────
  lib/*           Math: portable algorithm, zero `use reovim_arch*`
  system/lib/kernel
                  World bridge: common boot/system assembly, console/FDT/
                  device policy; imports `uapi/*` up-face and `kabi/*`
                  down-face; never imports arch/provider crates
  ─────────────── contract line ───────────────
  kabi/platform   machine-services seam   (install-gated PlatformVtable)
  kabi/panic      fault-policy atoms      (always present, no install gate)
  kabi/device     edit-as-buffer seam
        │
        ▼ down-face — hardware/provider code imports `kabi/*`
  platform-{target}-{strategy}   provider; target-specific, no direct `uapi/*`
  arch-floor-{target}            lang items (#[panic_handler], _start)
  arch-sys-{target}              raw hardware mechanism (returns NATIVE)

  arch-sys-* → {}   no product source names arch; exactly three things
                    cross at LINK, not import: #[panic_handler],
                    #[global_allocator], and the _start→editor-entry symbol.
```

Reading the stack:

- **`uapi/posix` is canonical, not Linux-derived.** It owns the POSIX
  values; Linux/x86-64 numbers may happen to match some of them, but a
  hardware/provider crate does not import `uapi/posix` directly. The
  `uapi/*` crates are the portable product face; `system/lib/kernel` bridges
  those values to the `kabi/*` down-face.
- **Three down-face seams, one shape.** `kabi/platform` is the
  install-gated machine-services trap gate; `kabi/panic` is the
  always-present fault floor; `kabi/device` is the edit-as-buffer seam.
  Each is a frozen `#[repr(C)]` ops table, never a `dyn Trait`.
- **`#[vtable]`-style authoring.** A contract trait is the authoring DSL
  for an ops table; an in-tree macro lowers it to the frozen `#[repr(C)]`
  table plus per-slot `HAS_*` presence consts. The trait never becomes a
  runtime `dyn` seam.
- **Not a HAL.** The lower provider satisfies `kabi/*` and may translate
  native machine facts into provider-facing `kabi` values, but it does not
  know the product's `uapi/*` vocabulary. The `uapi`↔`kabi` bridge belongs
  in `system/lib/kernel`. A direct `uapi/posix` import below that bridge is
  forbidden unless a concrete inescapable case is found and recorded.

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

- Exact crate-edge listing, the four-tier edge model, and the
  `arch → {}` firewall rule → `01-Architecture/02-Project-Layout-and-DAG.md`.
- Composition-root scope → `01-Architecture/03-Apps-and-Invocation.md`.
- Client tier internals → `08-Client/01-Layer-Model.md`.
- ABI surface details → `06-ABI/01-Surface.md`.
- Platform-contract slots, `#[vtable]`-style ops macro, append-only vtable
  evolution → `06-ABI/05-Platform-Contract.md`.
- POSIX-provider role per OS mode and the `arch` hard-split rule →
  `01-Architecture/06-OS-Modes.md`.
- `kabi/device` edit-as-buffer seam → `04-Domain-Substrate/06-Device-Domains.md`.

## Open items

None — this chapter is a synopsis; its rules live downstream.

## Conformance

This chapter has no rules. Subordinate chapters carry `DAG*`, `AL*`,
`CL*` rules whose conformance fixtures live in
`09-Conformance/01-Rule-Matrix.md`.
