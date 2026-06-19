# 1.2 — Project Layout and Dependency DAG

**Scope.** Source-tree categories for every workspace crate, the
allowed and forbidden dependency edges, the depgraph probe model, and
drift detection.

**Heritage.** #789 target classes (DAG6).

**Locked rules.** `DAG1`, `DAG2`, `DAG3`, `DAG4`, `DAG5` (amended §9.1),
`DAG6`, `DAG7` (candidate, §12).

---

## 1. Source categories

Every non-archived workspace crate belongs to **exactly one**
category.

| Category | Paths | Role |
|---|---|---|
| Foundation | `arch/` (→ `arch-sys-*`, `arch-floor-*`), `platform-*`, `system/lib/kernel`, `lib/*`, `uapi/*`, `kabi/*` | Backend floor and its hard-split families (raw mechanism `arch-sys-*`, lang-item floor `arch-floor-*`, canonicalizing providers `platform-*`; §12), the World system kernel (`system/lib/kernel`) — the bare-metal device-neutral library + boot assembly that plays the `kabi/platform` provider role in RTOS mode, a contract *implementor* structurally peer to `platform-*` (§11), core libraries incl. `lib/ds` DS algorithms, up-face ABI (`uapi/*`), down-face platform/device/panic contracts (`kabi/*`). No upward deps. |
| Server contracts | `editor/lib/subsys/*` | Closed server contracts and safe wrappers. No ext deps. |
| Server kernel | `editor/lib/kernel/*` | Kernel mechanisms. No ext or client deps. |
| Server runtime | `editor/lib/server/*` | Framed-protocol and dispatch glue. |
| Client contracts | `client/lib/subsys/*` | Closed client contracts. No ext or platform deps. |
| Server extensions | `editor/{modules,drivers,providers,domains}` | Runtime-loaded server policy/mechanics. |
| Client extensions | `client/{platforms,drivers,modules,capabilities}` | Platform runtimes and client open implementations. |
| System extensions | `ext/system/drivers/*` | Build-time-linked machine drivers, registered via the in-tree `#[used]`-section registry (not runtime-loaded). World side, below the `kabi/device`/`kabi/platform` contracts (T3, §12). Distinct from the runtime-loaded `editor/drivers/*`. |
| Composition roots | `apps/*` | User-facing binaries and app libraries. |
| Tools | `tools/*` | Dev / test / perf only. Non-shipping. |
| Archive | `archive/*` | Non-normative heritage. Excluded from depgraph. |

The four `client/<category>/` are: `platforms`, `drivers`,
`modules`, `capabilities`. All plural.

**"driver" is three distinct families — qualify it always.**
`editor/drivers/*` (runtime-loaded server-policy cdylibs),
`client/drivers/*` (client open-implementation cdylibs), and
`ext/system/drivers/*` (build-time machine drivers, `#[used]`-section). The
"driver vtable" of `06-ABI/05-Platform-Contract.md` §3.1 refers to the
runtime-loaded cdylib seam, **not** the build-time system drivers, which
carry no hot-unload or generation fence.

## 2. Allowed category edges

Missing edges are forbidden. Each row is "may depend on the listed
categories".

| From | May depend on |
|---|---|
| `arch/` | `kabi/*` (implements the platform handle, §11), `lib/ds` (uses DS algorithms internally); otherwise `core` only — `arch/` is the backend floor (§9/§10) |
| `lib/*` | `arch/`, peer/lower `lib/*` (per §6 sub-DAG) |
| `lib/ds` | `kabi/*` only — reaches its backend through the boot-installed handle; **never names `arch`** (the `lib/ds ⊄ arch` firewall). Overrides the generic `lib/*` row. |
| `uapi/*` | `lib/*` only when the dep is target-neutral and ABI-safe; no server/client/ext/apps |
| `kabi/*` | nothing impl-side — `core` + target-neutral `lib/*` only; declares the down-face contract and owns the platform-handle global static and `AllocError`; **no `kabi → arch`** |
| `editor/lib/subsys/*` | `uapi/*`, `lib/*`, `arch/`, allowed peer subsys edges |
| `editor/lib/kernel/*` | `editor/lib/subsys/*`, `uapi/*`, `lib/*`, `arch/` |
| `editor/lib/server/*` | `editor/lib/kernel/*`, `editor/lib/subsys/*`, `uapi/protocol`, `lib/*` |
| `client/lib/subsys/*` | `uapi/*`, `lib/*`, `arch/`, allowed client-subsys DAG edges |
| `editor/*` | `editor/lib/subsys/*`, `uapi/*`, `lib/*`; named ext peers per manifest |
| `client/platforms/*` | `client/lib/subsys/*`, `uapi/*`, `lib/*` |
| `client/drivers/*` | `client/lib/subsys/*`, `uapi/*`, `lib/*`, optional platform/capability contracts |
| `client/modules/*` | `client/lib/subsys/*`, `uapi/*`, `lib/*`, capability contracts; no platform IO except via capabilities |
| `client/capabilities/*` | `client/lib/subsys/*`, `uapi/*`, `lib/*`, peer capability sub-DAG |
| `apps/server` | server runtime/contracts/foundation only; no concrete client extension crates |
| `apps/tui`, `apps/web` | selected `client/platforms/<p>` plus client contracts/foundation |
| `apps/cli` | protocol/client CLI support; no server or extension implementation stack |
| `apps/reovim` | sibling app library targets and selected platform runtime under explicit `embedded-*` feature gates |
| `tools/*` | any in-repo crate if non-shipping and named in the probe catalog |

Every "may depend on" grant above is scoped to **in-repo crates**;
no row grants a third-party crate (`DAG5`, §9).

## 3. Forbidden category edges

| From | Must not depend on |
|---|---|
| **Every category** | third-party crates, in any dependency table (`DAG5`, §9) |
| Foundation | `editor/*`, `client/*`, `ext/*`, `apps/*`, `tools/*` |
| Server contracts | `ext/*`, `apps/*`, concrete Domain/provider modules |
| Server kernel | `ext/*`, `apps/*`, `client/*`, server runtime, concrete Domain/provider modules |
| Client contracts | `client/*`, `apps/*`, server kernel/runtime internals |
| Server extensions | `apps/*`, client platform/runtime crates, server kernel internals |
| Client extensions | `apps/*`, server kernel/runtime internals, concrete server extension crates |
| Non-launcher apps | sibling app crates |
| Launcher | plugin/package implementation logic; direct extension crates except the explicit platform-runtime composition edge |

## 4. Locked rules

> **DAG1 — Source categories are closed.** Every non-archived
> workspace crate belongs to exactly one source category in §1.
> Unknown or ambiguous crate paths fail depgraph.
> *Class*: CI/depgraph.

> **DAG2 — Dependency graph is closed by default.** A crate may
> depend only on categories explicitly listed in §2. Missing edges
> are forbidden.
> *Class*: CI/depgraph.

> **DAG3 — Composition-root exceptions are explicit.** `apps/*` and
> `tools/*` may wire layers together only through named composition
> edges covered by the probe catalog. Implicit app-to-ext or
> app-to-sibling edges are forbidden.
> *Class*: CI/depgraph.

> **DAG4 — Runtime install layout grants no Cargo edge.** A cdylib
> installed under `$ROOT/{module,driver,capability}/*` may be
> discovered at runtime; that does not imply any static Cargo edge
> is allowed.
> *Class*: spec-asserted.

> **DAG5 — Dependency graph is sovereign.** No third-party crate
> appears anywhere in the workspace dependency graph: not in
> `[dependencies]`, not in `[build-dependencies]`, not in
> `[dev-dependencies]`, of any workspace crate. The only permitted
> dependencies are `std`/`core`/`alloc` and in-repo `path`
> dependencies. The Rust toolchain is the only trusted external (§9).
> OS interfaces are reached exclusively through hand-written
> `extern "C"` FFI bindings owned by `arch/`.
> *Class*: CI/depgraph.

> **DAG6 — Every product crate is no_std + no_alloc; the platform
> floor lives in `arch/`.** Every shipped crate carries `#![no_std]`
> at its crate root, **including `arch/`**. The toolchain `std` AND
> `alloc` crates are both forbidden in product code. Binaries are
> `#![no_main]`; process entry is an `arch/`-owned `_start`. The
> workspace profile is `panic = "abort"`; unwinding is not part of
> the product ABI. `arch/` owns the platform floor (§10); each
> target binds to its lowest stable boundary per the target-class
> table in §10. The two transitional bootstrap states in §10 are
> tracked and ratchet to zero.
> *Class*: CI/depgraph + build constraint.

## 5. Depgraph probe model

The probe at `lib/depgraph/tests/` runs once per `cargo test` and:

1. Enumerates every workspace crate (excluding `archive/*`).
2. Classifies each by §1. Unknown path → **fail**.
3. For every classified crate, walks `Cargo.toml` deps and verifies
   each edge against §2. Missing-edge violation → **fail**.
4. For every `apps/*` and `tools/*` crate, verifies composition
   edges against the probe catalog (§7).
5. **Sovereignty walk (`DAG5`)**: parses all three dependency tables
   (`[dependencies]`, `[build-dependencies]`, `[dev-dependencies]`)
   of every workspace `Cargo.toml`; any dependency that is not an
   in-repo `path` dependency → **fail**. A second, resolved-graph
   gate verifies the lockfile/`cargo metadata` package set contains
   only workspace-local packages → any registry or git source
   **fails**.
6. **Zero-std walk (`DAG6`)**: verifies every product crate root
   declares `#![no_std]`; sweeps product sources for
   `extern crate std`, `use std::`, `extern crate alloc`, and
   `use alloc::` → any hit **fails**. Test targets and the §10
   bootstrap-state crates are the only exclusions, and each
   exclusion is enumerated in the probe, not pattern-matched.
   A profile gate verifies workspace `panic = "abort"`.
7. Reports a summary: per-category crate count, ambiguous-classification
   count (must be zero), violation list.

Probes that already enforce subsets of these rules:
- `client_subsys_no_ext.rs`
- `client_subsys_dag.rs`
- `ext_client_flat_categories.rs`
- `ext_client_category_isolation.rs`
- `apps_bin_client_scope.rs`
- `core_ext_boundary.rs`

## 6. Foundation sub-DAG (TODO)

The category-level DAG forbids upward edges from `arch/` and `lib/*`
but does not specify peer ordering inside foundation. The split
chapter must enumerate which `lib/*` crates may depend on which
others (e.g. `lib/depgraph` may depend on `lib/dylib-loader`?).
Without this enumeration, "selected `lib/*`" in §2 is informal.

## 7. Composition-root catalog (TODO)

`apps/*` exception edges must be enumerated by exact crate name and
feature flag. Example shape:

```toml
# tools/depgraph-probes/composition-edges.toml
[[edge]]
from   = "reovim"                          # apps/reovim
to     = "reovim-tui"                      # apps/tui as library
gate   = "embedded-tui"
reason = "Embedded-default mode boots TUI in-process."
```

The probe consumes this file as the only authoritative source of
composition exceptions. Edges not in the catalog fail DAG3. Catalog
entries may name only in-repo crates — the catalog cannot grant a
third-party edge (`DAG5`).

## 8. Drift allowlist (transitional)

Known transitional violations are recorded in a time-bounded
allowlist with issue references and expiry criteria:

```toml
# tools/depgraph-probes/transitional-allowlist.toml
[[entry]]
from    = "apps/server/src/bootstrap.rs"
to      = "reovim-driver-display"
reason  = "Theme/gutter/statusline imports pending #775."
issue   = "#775"
expires = "v0.16.0"
```

Entries cannot grow silently; the probe fails on any edge not in
either the allowed list (§2) or the dated allowlist. The allowlist
covers category-edge drift only — it cannot admit a third-party
dependency (`DAG5` has no allowlist).

## 9. Dependency sovereignty

Motivated by the North Star (README): reovim is built to survive 50
years, and the spec — not any dependency — is the wire truth.

**The law (`DAG5`).** The workspace dependency graph is closed:

- The only permitted dependencies of any workspace crate are
  `std`/`core`/`alloc` and in-repo `path` dependencies. This applies
  to all three Cargo dependency tables — `[dependencies]`,
  `[build-dependencies]`, and `[dev-dependencies]`. Test-only and
  build-time third-party crates are forbidden exactly like shipped
  ones.
- No registry (crates.io), git, or vendored-foreign source appears in
  the resolved graph. Vendoring a third-party crate into the repo
  does not satisfy this rule — sovereignty means code we wrote and
  specced, not code we copied.

**Toolchain-trust boundary.** The Rust toolchain — `rustc`, `cargo`,
the standard library, and toolchain-level subcommands (`rustfmt`,
`clippy`, `cargo-llvm-cov`) — is the only trusted external. Toolchain
tools are invoked as programs; they are never graph dependencies, so
they do not violate the closed graph.

**The FFI floor.** Every OS interface — `dlopen`/`dlsym`, sockets and
UDS, `termios` and terminal ioctls, clocks, file and process
primitives beyond `std` — is reached through hand-written
`extern "C"` bindings owned by `arch/`. Higher layers consume `arch/`
wrappers; no other crate declares foreign OS bindings. `arch/` is the
single, auditable seam between reovim and the platform.

**Enforcement.** The §5 sovereignty walk (manifest-level) plus the
resolved-graph gate (lockfile/metadata-level), both fail-closed, both
run in CI on every PR. There is no allowlist for `DAG5` (§8).

> Note (non-normative): the rule's consequences elsewhere in the
> spec — the in-house framed wire protocol (7.3), `arch/`-realized
> concurrency contracts (2.1, 2.3, under `DAG6` §10), the
> raw-terminal layer (8.2), and hand-rolled dynamic loading (2.2) —
> each carry a heritage note at the owning chapter.

### 9.1 The "include tool" clause (amendment)

**Amendment — deliberate, stricter, never weaker.** Sovereignty extends to
**host and build tooling**, not only shipped product. No in-repo tool
(`tools/*`, `lib/depgraph`, build scripts, and any future composition or
Kconfig-style generator) may take a third-party dependency: tooling is
in-tree code held to the same closed-graph law as product crates. In
particular:

- **Distributed-slice / registry crates are forbidden as dependencies.**
  `linkme` and `inventory` are the tempting exception for the
  `#[used]`-section system-driver registry (§12;
  `01-Architecture/06-OS-Modes.md` §6.1) — both are forbidden. The registry
  is realized with the in-tree `#[used]` / `#[link_section]` mechanism over
  the linker's `__start_/__stop_` section symbols, no external crate.
- **Composition tooling is in-tree.** Any Kconfig-style provider/config
  generator for Kbuild-style composition is a `tools/*` crate written in
  in-repo Rust, never a pulled-in build framework.

**Rationale.** A 50-year artifact that bans ambient runtime dependencies but
trusts an ambient *build* dependency has only moved the supply-chain surface,
not closed it. The toolchain-trust boundary above admits exactly
`rustc`/`cargo` and the toolchain subcommands as *invoked programs*; it
admits nothing as a graph dependency — tool or product. This clause is
consistent with the root `Cargo.toml` marker (`[workspace.dependencies]`
intentionally absent — closed to `std` + in-repo path deps) and is
orthogonal to `DAG6`'s std-in-tooling bootstrap rows (§10), which track a
`std` → `no_std` *migration* of in-tree tools, not an external-dependency ban.

## 10. Zero-std sovereignty

Motivated by the North Star (README): the reliability grade is a
Mission to Mars, and the operating maxim is "test what you fly, fly
what you test". A product whose every layer above the syscall surface
is in-repo, `core`-only code has exactly two foundations to certify —
the Rust `core` language and the OS — and no third, ambient runtime
whose behaviour the spec does not own.

**The law (`DAG6`).**

- Every shipped/product crate carries `#![no_std]` at its crate
  root, **including `arch/`**.
- The toolchain `std` AND `alloc` crates are both forbidden in
  product code: `extern crate std`, `use std::`, `extern crate
  alloc`, and `use alloc::` are all violations.
- Binaries are `#![no_main]`; process entry is an `arch/`-owned
  `_start`.
- The workspace profile is `panic = "abort"`; unwinding is not part
  of the product ABI (6.2 `AB12` realizes panic containment without
  unwinding).

**The platform floor (`arch/`).** `arch/` is the single crate that
owns, in-repo:

- platform FFI — each target binds to its **lowest stable boundary**,
  defined by target class:

  | Target class | Example targets | Stable boundary | `arch/` FFI form |
  |---|---|---|---|
  | **kernel-ABI** | Linux (x86_64, aarch64) | The kernel syscall ABI | Raw syscall FFI (`asm!`/`naked_asm!`); libc is **FORBIDDEN** — the syscall ABI is the stable layer, and interposing libc would add an unsovereign runtime dependency |
  | **system-library** | macOS, illumos, Windows | The vendor system library (stable, versioned OS API) | Hand-written `extern "C"` bindings to the system library; remains `no_std` and dependency-sovereign under `DAG5` |
  | **freestanding** | bare metal | Hardware and firmware interfaces | Hand-written `extern "C"` or `asm!` bindings to hardware/firmware; no OS ABI |

  For kernel-ABI targets the rule's effect is the same as the
  previous text: libc remains forbidden on Linux. The target-class
  table generalises the floor rule to future OS ports without
  weakening the Linux constraint. Backend module placement and
  cfg-selection conventions follow the per-target backend structure
  at the end of this section.

- process entry (`_start`) and process exit,
- the panic handler,
- the allocator **backend** (raw memory: a static arena +
  `mmap`/`brk`-class syscalls),
- the **backend primitives** of threads and sync — the
  `park`/`unpark`/futex-class operations a mutex/rwlock/condvar blocks on.

`arch/` owns the **effectful backend primitives**, not the data
structures built on them. The heap-DS **algorithms** — the growable
sequence, hash map, owned byte string, shared-reference, and the
mutex/rwlock/condvar state machines — are portable, target-neutral
**Math** and live in `lib/ds`. They reach their backend through the
boot-installed `kabi` platform handle (`06-ABI/05`), never by naming
`arch`. This is the Math/World airlock applied to the type system: a
value's *layout and algorithm* are identical on every provider (Math);
the *allocator and park/unpark behind it* are provider-specific (World),
injected at `Init::boot`. (Supersedes the prior "`arch/` owns ALL heap
data structures" rule, which coupled the portable algorithm to the
World backend.)

The project rule, stated plainly: *the effectful primitive is created at
`arch/`; the data structure that uses it is created at `lib/ds` against
the `kabi` contract.* Crates above the floor name `lib/ds` + `kabi`,
never `arch`; no crate other than `arch/` declares foreign OS bindings
(§9) or backend primitives.

**`uapi/protocol` is zero-allocation.** The wire-protocol crate
(7.3 `SP15`) performs no allocation at all — callers provide every
buffer (`encode(&self, &mut [u8])`). It therefore needs NO `arch/`
data-structure edge; its only dependencies are `core` and `uapi`
siblings. This keeps the frozen ABI surface free of any platform
coupling.

**Illustrative-collection clause.** Where any chapter writes
`Vec<T>`, `HashMap<K, V>`, `String`, `VecDeque<T>`, or `Arc<T>` in an
illustrative struct listing, it denotes the `lib/ds`-provided heap DS
of the same contract (growable sequence / hash map / owned byte
string / ring-capable sequence / shared reference). The `std`/`alloc`
types themselves are forbidden in realization (`DAG6`); the familiar
names are kept in spec prose because their *contracts* are what the
chapters bind.

**Transitional bootstrap states** (tracked; target = zero; these are
sequencing necessities — `arch/` does not exist yet — NOT
convenience exemptions; per the North Star, developer convenience is
never a valid justification):

| Bootstrap state | Why it exists | Migration target | Migration phase |
|---|---|---|---|
| cargo's libtest harness links `std` in integration-test orchestration (`arch/tests/fixtures_exec.rs`) and ground-support tooling (`lib/depgraph`, `scripts/`); `arch/` crates themselves are `#![no_std]`; the arch unit-test suite runs on the in-repo no_std runner (`arch-selftest`, feature `selftest`+`runtime`) | arch unit tests are now on the in-repo runner (landed #785 Phase 5); remaining libtest: `lib/depgraph` (state 2), `arch/tests/fixtures_exec.rs` (exec orchestration only — a tiny std harness that execs no_std binaries) | migrate `arch/tests/fixtures_exec.rs` orchestration onto an arch-native exec harness; migrate `lib/depgraph` onto `arch/` fs/process when those land | the process-lifecycle phase (lands `arch/` fs/process) |
| `lib/depgraph` + `scripts/` (ground-support tooling) use `std` | `arch/` fs/process APIs do not exist yet | migrate onto `arch/` fs/process APIs, so the tooling dogfoods the platform floor | the process-lifecycle phase (lands `arch/` fs/process) |

**Verification doctrine.** The in-repo `no_std` test runner over
`arch/` is the `DAG6` realization of "test what you fly": test
binaries and flight binaries share one platform floor — same entry,
same allocator, same panic path. The libtest bootstrap exists only
until it lands, and the bootstrap table above is the only place such
states may be recorded.

**Enforcement.** The §5 zero-std walk (`#![no_std]` presence;
`std`/`alloc` usage sweep; `panic = "abort"` profile gate),
fail-closed, no allowlist except the two tracked bootstrap states
above — which are enumerated in the probe and ratchet to zero.

**Per-target backend convention (`arch/src/sys/`).** The platform
floor is decomposed into per-target backend modules, cfg-selected from
`arch/src/sys/mod.rs`:

- Each target backend lives at `arch/src/sys/<target>/` (e.g.
  `linux_x86_64`, `linux_aarch64`, the freestanding `none_aarch64`),
  selected by `#[cfg(all(target_os = "...", target_arch = "..."))]`; all
  backends expose identical floor function signatures — no traits, no
  vtables. The wrapper-level floor surface is re-exported through the
  selected backend (one `pub use target::{…}` list in `sys/mod.rs`), so
  a backend that fails to provide an item is a compile error, not a
  runtime gap.
- All target-specific `asm!`/`naked_asm!` is confined to
  `arch/src/sys/<target>/` (raw syscall primitives and the fused clone
  trampoline) and to the cfg-gated `_start` entry arms in
  `arch/src/start.rs`. No inline asm may appear elsewhere in `arch/`.
- `wrap.rs` at the `sys/` level is Linux-kernel-ABI family code shared
  by Linux backends only (cfg-gated to `target_os = "linux"`).
  Non-Linux and freestanding backends provide their own wrap-level
  implementations behind the same floor signatures — `none_aarch64`
  realizes them over PL011 MMIO, the generic timer, and a static page
  arena — and nothing in the per-target structure assumes every backend
  routes through the shared Linux wrap layer. `errno.rs` is
  target-neutral floor vocabulary (every backend's wrappers return the
  same `Errno` values); the raw-syscall re-export (`syscall0..6`) is
  kernel-ABI-class surface, exposed for Linux targets only.
- Targets with no registered backend fail at compile time with an
  explicit `compile_error!` fallback arm in `sys/mod.rs`.

The asm-confinement rule is enforced by a source-grep probe in
`lib/depgraph/tests/` (added #790).

## 11. Kernel / platform seam (candidate edges)

The three-kernel model (`02-Process/01-Kernel-Types.md` §0) reaches its
substrate through two **down-face contracts** in `kabi/` — `kabi/platform`
(`06-ABI/05-Platform-Contract.md`) and `kabi/device`
(`04-Domain-Substrate/06-Device-Domains.md` §8). `kabi/` *declares* the
mechanism; `arch/` and the system kernel *implement* it — dependency flows
`arch → kabi`, never `kabi → arch` (a contract never depends on its
implementor; that would run backward through the airlock). The edges below
are **candidate rules**: they become depgraph-enforced only when the crates
exist (`kabi/platform`, `kabi/device`, and the system/client kernel crates).
Until then this section is the target the scaffold builds toward, not a gate.

| Target rule | Edge |
|---|---|
| kernels depend on the contract, not the floor | `editor-kernel → kabi/platform`; `client-kernel → kabi/platform`; data structures via `lib/ds`; **no** `*-kernel → arch` |
| one owner of the floor, per mode | impl edges `arch(hosted) → kabi/platform` and `system-kernel → kabi/platform`; plus `system-kernel → arch` |
| client two-substrate | `client-kernel → uapi/protocol` is the only cross-kernel edge; **no** direct `client-kernel → editor-kernel` |
| device bridge is a closed contract | `editor/domains/* → kabi/device`; provider impls `arch(hosted) → kabi/device` and `system-kernel → kabi/device`; **no** `domain → driver` or `domain → arch` |
| client I/O is capability-mediated | `client-kernel → render`/capability subsys; client I/O drivers bottom out on `kabi/device` (bare metal) or platform fd-I/O (hosted); **no** `client-kernel → arch`, **no** `client-kernel → driver` |

Two of these are already locked under existing rules and need no new
machinery: "drivers are loaded, not linked" is **DAG4** (§4) — each kernel
binds drivers by vtable, never by Cargo edge; "no client platform IO except
via capabilities" is the `client/modules/*` row of §2.

**Supersession (the one real change to §2).** Row `editor/lib/kernel/*` in
§2 currently grants `→ arch`. When `kabi/platform` lands, that grant is
**removed**: the editor kernel will name only `kabi/platform` (and `lib/ds`
for data structures), and a `*-kernel → arch` edge becomes a depgraph
violation. This is deferred, not applied now — flipping it before
`kabi/platform` exists would forbid a real current edge. The candidate rule
is recorded here so the supersession is not rediscovered during
implementation.

## 12. The four-tier model and the `arch` firewall (candidate, `DAG7`)

The Foundation tier (§1) resolves into **four dependency tiers**, ordered by
what each may name. This generalizes the existing edge rules (§2, §10, §11)
to the post-split `arch-*` / `platform-*` crate families
(`01-Architecture/06-OS-Modes.md` §2.2):

| Tier | Crates | May name | Must not name |
|---|---|---|---|
| **T1 — Contracts** | `uapi/*`, `kabi/*` | `core`, target-neutral `lib/*` | any `arch-*`, `platform-*` (`DAG7`) |
| **T2 — Math** | `lib/*` (incl. `lib/ds`) | `core`, `kabi/*`, peer `lib/*` | any `arch-*`, `platform-*` (the `lib/ds ⊄ arch` firewall, §10) |
| **T3 — Providers + floor** | `arch-sys-*`, `platform-*`, `arch-floor-*`, `ext/system/drivers/*` | `kabi/*` (implements it), `lib/ds`, `core` | upward: no contract, kernel, or app |
| **T4 — Composition roots** | `apps/*`, boot images | any tier, by mode-selected config | — |

**The arch hard-split (T3).** `arch/` matures into three crate families by
the three-knowledges rule (OS-Modes §2.2): `arch-sys-*` (raw hardware,
returns NATIVE), `platform-*` (the canonicalizing provider, the POSIX trap
gate), and `arch-floor-*` (lang items). `ext/system/drivers/*` are the
build-time, `#[used]`-section-registered machine drivers a
`platform-*`/system kernel composes — World side, below `kabi/device`,
**not** runtime-loaded.

**`DAG7` — the Tier-1 firewall (candidate).** No `uapi/*` or `kabi/*` crate
names any `arch-*` or `platform-*` crate. This extends the existing
`no kabi → arch` rule (§11) to the post-split family names: a contract never
names a provider or a floor, by import. Today
`lib/depgraph/tests/firewall_probe.rs` asserts only the Tier-2 residual
(`lib/ds ⊄ arch`); the Tier-1 assertion is **spec-first** — the probe
extension lands in the 05d sub-plans, exactly as `DAG6` was spec-first
before its enforcement flight.

**`arch → {}`, stated whole.** Taking §2 (no upward edges), §10
(`lib/ds ⊄ arch`), §11 (`no *-kernel → arch`), and `DAG7` together: **no
contract, Math, kernel, or app crate names `arch` by import** — the import
residual of `arch` in product source is the empty set, panic included.
Exactly three things cross from the floor to the product at **LINK, not
import**: `#[panic_handler]`, `#[global_allocator]`, and the
`_start → editor-entry` extern symbol
(`01-Architecture/01-Layer-Model.md` §1.1). Everything else the product
needs from the machine arrives through a `kabi` contract, never an `arch`
name. (Resolved-closure still reaches the floor transitively via `lib/ds`'s
backend; like §11, this is a *direct-import* rule, stated at the strength it
holds.)

## Open items

1. **Facade re-export vs the contract tier — RESOLVED (mechanism model).**
   Superseded: the platform contract does **not** re-export `arch`'s value
   types (that ran backward through the airlock — a `kabi → arch` edge, and a
   cycle with `arch → kabi`). Instead `kabi/platform` *declares* the mechanism
   and `arch` *implements* it (`arch → kabi`); the DS algorithms live in
   `lib/ds` (§10), reaching the backend through the boot-installed handle, so
   no `kabi → arch` edge exists. The platform-handle global static and
   `AllocError` live in `kabi`. The "`kabi/platform` has zero impl-side deps"
   goal is met by construction.
2. Foundation sub-DAG enumeration (§6) — deferred to the workspace
   scaffold, `TODO(#778)`.
3. Composition-root catalog format and probe wiring (§7) — deferred
   to the workspace scaffold, `TODO(#778)`.
4. Transitional allowlist current contents (§8) — resolved #782: the
   v0.16 rebuild starts greenfield; the allowlist starts **empty**
   (the #775 driver-display entry belonged to the archived v0.15
   tree).
5. `tools/*` forbidden-edge table — partially resolved #782: `DAG5`
   already binds `tools/*` to in-repo crates only; the remaining
   per-category edge table is deferred to the workspace scaffold,
   `TODO(#778)`.

## Conformance

| Rule | Fixture |
|---|---|
| DAG1 | Probe over current workspace; fixture adds a crate at an unrecognised path; expect **fail**. |
| DAG2 | Fixture adds a `lib/*` crate depending on `server/*`; expect **fail**. |
| DAG3 | Fixture adds `apps/cli → reovim-driver-display`; expect **fail**. |
| DAG4 | Inspect runtime install paths; verify no probe edge created. |
| DAG5 | Fixture crate adds a registry dependency in each of the three dep tables in turn; expect **fail** for all three. Resolved-graph gate: lockfile/metadata package set is exactly the workspace-local set; a registry or git source → **fail**. Include-tool clause (§9.1): a `tools/*` or `lib/depgraph` crate adding a third-party dep (e.g. `linkme`, `inventory`) → **fail**. |
| DAG6 | Three negative fixtures: a product crate missing `#![no_std]` at its root; a `use std::` in product code; a `use alloc::` in product code — each expects **fail**. Profile gate: workspace `panic` setting other than `"abort"` → **fail**. (Probe lands with the L10 enforcement flight; spec-first until then, like DAG5 pre-scaffold.) |
| DAG7 | Fixture: a `uapi/*` or `kabi/*` crate adds an `arch-*`/`platform-*` path dep; expect **fail**. Extends `firewall_probe.rs` from the Tier-2 residual (`lib/ds ⊄ arch`) to the Tier-1 contracts; spec-first until the 05d probe flight, like DAG6. |
