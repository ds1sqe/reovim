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
│ FOUNDATION    uapi/* · kabi/* · lib/* · platform-* · arch-*     │
└─────────────────────────────────────────────────────────────────┘
```

### 1.1 Foundation stack (up-face / down-face)

The FOUNDATION tier is itself layered. It is not a flat bag of crates; it
is a POSIX personality with a contract face pointing up at the product and
a provider face pointing down at the machine.

```
        ▲ up-face — the product imports the contract
        │
  uapi/posix      canonical POSIX values (OpenFlags, Mode, Fd, Errno);
                  owns the numbers — no reference provider supplies them
  uapi/abi        module/plugin cdylib ABI surface
  uapi/protocol   framed client↔server protocol surface
  ─────────────── contract line ───────────────
  lib/*           Math: portable algorithm, zero `use reovim_arch*`
  ─────────────── contract line ───────────────
  kabi/platform   machine-services seam   (install-gated PlatformVtable)
  kabi/panic      fault-policy atoms      (always present, no install gate)
  kabi/device     edit-as-buffer seam
        │
        ▼ down-face — a provider installs the implementation
  platform-{target}-{strategy}   canonicalizing provider (POSIX trap gate)
  arch-floor-{target}            lang items (#[panic_handler], _start)
  arch-sys-{target}              raw hardware mechanism (returns NATIVE)

  arch-sys-* → {}   no product source names arch; exactly three things
                    cross at LINK, not import: #[panic_handler],
                    #[global_allocator], and the _start→editor-entry symbol.
```

Reading the stack:

- **`uapi/posix` is canonical, not Linux-derived.** It owns the POSIX
  values; Linux/x86-64 numbers happen to fill them, so the Linux
  provider's mapping is identity. `uapi/posix` is the portable contract;
  `uapi/abi` and `uapi/protocol` are the other two up-face surfaces and
  are unrelated to it.
- **Three down-face seams, one shape.** `kabi/platform` is the
  install-gated machine-services trap gate; `kabi/panic` is the
  always-present fault floor; `kabi/device` is the edit-as-buffer seam.
  Each is a frozen `#[repr(C)]` ops table, never a `dyn Trait`.
- **`#[vtable]`-style authoring.** A contract trait is the authoring DSL
  for an ops table; an in-tree macro lowers it to the frozen `#[repr(C)]`
  table plus per-slot `HAS_*` presence consts. The trait never becomes a
  runtime `dyn` seam.
- **Not a HAL.** The provider canonicalizes (impedance-matches the
  machine's native ABI to the POSIX contract) in one place; `arch-sys-*`
  returns the machine's NATIVE values and knows nothing of the contract.
  The split keeps `kabi` above `arch` — providers sit below `kabi`, not
  beside it.

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
