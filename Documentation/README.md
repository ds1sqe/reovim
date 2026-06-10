# Reovim Spec v4

**Status:** Draft v0 — chapters in flight.
**Date:** 2026-04-28.

v4 preserves v3's architecture and normalises the boundaries. Kernel
remains mechanism; Domains, modules, drivers, and client modules own
policy. Every byte, pointer, service, config slice, and cdylib
registry row that crosses a stable boundary gets explicit ownership,
versioning, validation, resource-limit, and observability rules.

This README is a thin navigation map. Substantive content lives in the
chapter files. The chapter file is the authority; this README links.

## North Star

Reovim is built to two ends: the **fastest-reaction editor**, and an
editor that **survives 50 years**.

Frameworks and RPC stacks die as a category; what survives on that
horizon is the OS syscall surface, terminal escape codes, and protocol
*documents*. This spec is therefore written to be the **wire truth**:
every surface of reovim — the framed wire protocol, the cdylib ABI,
the config and log formats — is specified completely enough that a
stranger, decades from now, can reimplement either side of any
boundary from these chapters alone.

Two structural consequences:

- **Dependency sovereignty (`DAG5`, 1.2 §9).** The workspace
  dependency graph is closed: `std`/`core`/`alloc` and in-repo crates
  only — no third-party crates in any dependency table, tests
  included. The Rust toolchain is the only trusted external. OS
  interfaces are reached through hand-written `extern "C"` bindings
  owned by `arch/`. Reovim owns both ends of every wire it speaks, so
  ecosystem interop buys nothing; durability buys everything.
- **Latency is a design driver, not an optimization pass.** Owning
  the wire, the runtime, and the terminal layer means no foreign
  scheduler, codec, or abstraction sits between a keystroke and the
  screen.

## Posture

- **v4 is a quality gate, not a feature pass.** Hardening contracts
  beats expanding scope.
- **Do not break userspace. The released ABI is eternal (AB15).**
  Post-v1.0, every shipped stable surface is supported forever;
  deprecation is allowed, removal is not. Pre-1.0 is the only
  window for clean breaks — v4 spends it deliberately (AB14).
- **Kernel ABI is `#[repr(C)]`, opaque handles, primitives, byte
  slices.** No Rust trait objects cross cdylib boundaries.
- **Domain is an ID, not a trait.** `DomainId(NonZeroU32)`; behaviour
  routes through tables.
- **Kernel input is key-free.** Keymaps, modes, motions, registers,
  leaders are Domain/client policy.
- **Runtime-loaded cdylibs are trusted native code.** v4 hardens
  loading, lifecycle, ABI, and observability — not memory sandboxing.

## Chapter index

| § | File | Subject |
|---|---|---|
| — | [`00-Vocabulary.md`](00-Vocabulary.md) | Cross-chapter terms |
| 1.1 | [`01-Architecture/01-Layer-Model.md`](01-Architecture/01-Layer-Model.md) | Foundation/contracts/kernel/runtime/ext/apps tiers |
| 1.2 | [`01-Architecture/02-Project-Layout-and-DAG.md`](01-Architecture/02-Project-Layout-and-DAG.md) | Source categories + dependency DAG (`DAG1..DAG4`) |
| 1.3 | [`01-Architecture/03-Apps-and-Invocation.md`](01-Architecture/03-Apps-and-Invocation.md) | Binaries, embedded vs subprocess, library root |
| 1.4 | [`01-Architecture/04-Provenance.md`](01-Architecture/04-Provenance.md) | Spec ownership of generated artefacts |
| 1.5 | [`01-Architecture/05-Configuration.md`](01-Architecture/05-Configuration.md) | Config service: participants, layers, trust, schemas (`CFG`) |
| 2.1 | [`02-Process/01-Kernel-Types.md`](02-Process/01-Kernel-Types.md) | `Kernel` and `Init` shapes, registries |
| 2.2 | [`02-Process/02-Lifecycle.md`](02-Process/02-Lifecycle.md) | Boot, cdylib load/unload, drain ordering (`LF`) |
| 2.3 | [`02-Process/03-Concurrency.md`](02-Process/03-Concurrency.md) | Turn gate + state lock, RefGuard (`CC`) |
| 2.4 | [`02-Process/04-Persistence.md`](02-Process/04-Persistence.md) | State persistence and restore |
| 3.1 | [`03-State/01-State-Split.md`](03-State/01-State-Split.md) | Kernel / Session / Module-state boundaries |
| 3.2 | [`03-State/02-Module-State.md`](03-State/02-Module-State.md) | View slots, opaque module state |
| 3.3 | [`03-State/03-Service-Registry.md`](03-State/03-Service-Registry.md) | Service descriptors, leases (`SVC`) |
| 3.4 | [`03-State/04-Registers.md`](03-State/04-Registers.md) | Register storage |
| 4.1 | [`04-Domain-Substrate/01-Domain.md`](04-Domain-Substrate/01-Domain.md) | `DomainId` and `DomainRouter` |
| 4.2 | [`04-Domain-Substrate/02-Domain-Tree.md`](04-Domain-Substrate/02-Domain-Tree.md) | Attachments, focus, pending (`DT`) |
| 4.3 | [`04-Domain-Substrate/03-Undo.md`](04-Domain-Substrate/03-Undo.md) | Undo/redo and byte-edit origin |
| 4.4 | [`04-Domain-Substrate/04-Stream.md`](04-Domain-Substrate/04-Stream.md) | Stream substrate (`S`) |
| 4.5 | [`04-Domain-Substrate/05-Coordination.md`](04-Domain-Substrate/05-Coordination.md) | Position/cursor carriers, codecs (`CR`) |
| 5.1 | [`05-View/01-Windows.md`](05-View/01-Windows.md) | Window tree, layout |
| 5.2 | [`05-View/02-Raw-Input.md`](05-View/02-Raw-Input.md) | `RawInput { kind, payload }` hot path |
| 5.3 | [`05-View/03-Projections.md`](05-View/03-Projections.md) | Projection contract |
| 6.1 | [`06-ABI/01-Surface.md`](06-ABI/01-Surface.md) | What is in/out of the ABI |
| 6.2 | [`06-ABI/02-Versioning-and-Vtables.md`](06-ABI/02-Versioning-and-Vtables.md) | `VtableHeader`, append-only, panic convention (`AB`) |
| 6.3 | [`06-ABI/03-Type-Catalog.md`](06-ABI/03-Type-Catalog.md) | The single source of `#[repr(C)]` layouts |
| 6.4 | [`06-ABI/04-Config-Slice.md`](06-ABI/04-Config-Slice.md) | `ConfigSlice` / `ConfigKvp` exact layout |
| 7.1 | [`07-Surfaces/01-Package-Manager.md`](07-Surfaces/01-Package-Manager.md) | `pkg sync`, lockfile, supply chain (`PM`, `SEC`) |
| 7.2 | [`07-Surfaces/02-Debug-Surface.md`](07-Surfaces/02-Debug-Surface.md) | Debug events, drive (`DS`) |
| 7.3 | [`07-Surfaces/03-Server-Client-Protocol.md`](07-Surfaces/03-Server-Client-Protocol.md) | Framed wire protocol (`SP`) |
| 7.4 | [`07-Surfaces/04-Config-CLI.md`](07-Surfaces/04-Config-CLI.md) | `reovim config dump|help|validate|set|force-set` |
| 7.5 | [`07-Surfaces/05-Log-CLI.md`](07-Surfaces/05-Log-CLI.md) | `reovim log` — the `dmesg` analog (`LOG9..LOG11`) |
| 8.1 | [`08-Client/01-Layer-Model.md`](08-Client/01-Layer-Model.md) | Client subsys / ext flat-category tree (`CL`) |
| 8.2 | [`08-Client/02-Platform-Runtimes.md`](08-Client/02-Platform-Runtimes.md) | TUI, web, native runners |
| 8.3 | [`08-Client/03-Drivers-Modules-Capabilities.md`](08-Client/03-Drivers-Modules-Capabilities.md) | Multi-vtable cdylibs, kinds |
| 8.4 | [`08-Client/04-Client-Debug.md`](08-Client/04-Client-Debug.md) | Client-side debug capability |
| 8.5 | [`08-Client/05-Packaging.md`](08-Client/05-Packaging.md) | Client manifests, distribution |
| 9.1 | [`09-Conformance/01-Rule-Matrix.md`](09-Conformance/01-Rule-Matrix.md) | Per-rule fixtures, golden tests (`CF`) |
| 9.2 | [`09-Conformance/02-Security.md`](09-Conformance/02-Security.md) | Threat model, transport auth (`SEC`) |
| 9.3 | [`09-Conformance/03-Failure-Modes.md`](09-Conformance/03-Failure-Modes.md) | Bounded drains, rollback (`FAIL`) |
| 9.4 | [`09-Conformance/04-Observability.md`](09-Conformance/04-Observability.md) | DS12 event taxonomy, correlation (`OBS`) |
| 9.5 | [`09-Conformance/05-Logging.md`](09-Conformance/05-Logging.md) | dmesg-style log rendering, ring, sink (`LOG`) |
| 10.1 | [`10-Development/01-Testing.md`](10-Development/01-Testing.md) | Development process: coverage, E2E smoke, goldens (`DEV`) |

Non-normative project history — the proposals and phase records that
shaped this architecture — lives in [`heritage/`](heritage/README.md).

## Locked-rule namespaces

| Prefix | Topic | Owner chapter |
|---|---|---|
| `L` | Legacy core; compatibility only | (carried from v3) |
| `LF` | cdylib lifecycle | 2.2 |
| `CC` | concurrency | 2.3 |
| `AB` | ABI / vtable / panic | 6.2 |
| `DT` | Domain tree + dispatch ordering | 4.1, 4.2 |
| `S` | Stream substrate | 4.4 |
| `PM` | Package manager | 7.1 |
| `DS` | Debug surface | 7.2 |
| `SP` | Server-client protocol | 7.3 |
| `AL` | Apps / launcher / invocation only | 1.3 |
| `DAG` | Source layout + dependency graph | 1.2 |
| `CFG` | Config service | 1.5, 6.4 |
| `CR` | Coordination carriers/codecs | 4.5 |
| `CL` | Client layer model | 8.1 |
| `SVC` | Service registry / leases | 3.3 |
| `PS` | Persistence / state files | 2.4 |
| `SEC` | Security / trust | 9.2 |
| `CF` | Conformance | 9.1 |
| `OBS` | Observability | 9.4 |
| `LOG` | Log rendering / ring / sink / reader | 9.5, 7.5 |
| `DEV` | Development process / testing requirements | 10.1 |
| `FAIL` | Failure modes | 9.3 |

`AL` is reserved for binaries and invocation. Config rules go to
`CFG`. Service rules go to `SVC`. Do not overload `AL`.

## Design decisions (2026-06)

The implementation-lock questions are all resolved; the adopted
decisions live in the owner chapters' rule bodies. One-line
summary per decision:

| Subject | Owner | Decision |
|---|---|---|
| `c_int` ↔ `ErrorCode` migration | 6.2 | Clean break: `ErrorCode` only (AB14); no compat loader — v0.15 impl archived, zero consumers |
| `ConfigSlice` compound encoding | 6.4 | `CanonicalToml` bytes, config-ABI v1 |
| Hot-path `RawInputKind` payloads | 5.2, 6.3 | USB HID keycodes + ext range; cell-primary mouse; first-class `Ime` kind; layouts in 6.3 §7 |
| Env-var canonical encoding | 1.5, 7.4 | Reversible kebab grammar (CFG10); spec swept to kebab-case |
| Handler/projector tie-break | 4.1, 4.2 | Canonically-sorted lockfile order + manifest-decl order (DT17) |
| Detach lifecycle ordering | 2.2, 2.4, 4.2 | detach → persist → subs → drops → release; leaf-first; non-blocking (LF12) |
| Manifest-kind enumeration | 6.2, 8.5, 4.4 | Nine-variant enum final; `StreamScheme` own kind; sub-kinds via vtable strings |
| Wire-form provenance | 1.4, 7.3 | 7.3's message inventory owns the wire form; `uapi/protocol` structs realize it; CI cross-check (CF5). Superseded the 2026-06 proto split under `DAG5` sovereignty. |
| Rule-index reshape pass | all chapters | Hybrid renumber; carried bodies restated in owner chapters; lock tiers renamed T1..T13 |
| v3 draft disposition | all chapters | Superseded; every carried rule restated here — the v3 draft tree is not retained (latest spec is the SSOT) |

Chapter lock is gated only by per-chapter promotion work
(conformance rows, CF1..CF5).

## Out of v4 target

- WASM browser runtime peer vs separate web SSR runner.
- Project package overlay trust prompt beyond `pkg sync` / `pkg verify`.
- Q-4 (cross-observer focus pairing) unless DomainTree requires it.
- Runtime config reload (boot-only in v4).

## How to read this spec

1. Skim this README for the chapter map and namespace.
2. Read [`00-Vocabulary.md`](00-Vocabulary.md) once.
3. Read chapters in numeric order; cross-references go forward.
4. For implementation, [`06-ABI/03-Type-Catalog.md`](06-ABI/03-Type-Catalog.md) is the only authoritative source of `#[repr(C)]` layouts.
