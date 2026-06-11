# 1.2 — Project Layout and Dependency DAG

**Scope.** Source-tree categories for every workspace crate, the
allowed and forbidden dependency edges, the depgraph probe model, and
drift detection.

**Heritage.** #789 target classes (DAG6).

**Locked rules.** `DAG1`, `DAG2`, `DAG3`, `DAG4`, `DAG5`, `DAG6`.

---

## 1. Source categories

Every non-archived workspace crate belongs to **exactly one**
category.

| Category | Paths | Role |
|---|---|---|
| Foundation | `arch/`, `lib/*`, `uapi/*` | Platform abstraction, core libraries, stable ABI/protocol/macro contracts. No upward deps. |
| Server contracts | `server/lib/subsys/*` | Closed server contracts and safe wrappers. No ext deps. |
| Server kernel | `server/lib/kernel/*` | Kernel mechanisms. No ext or client deps. |
| Server runtime | `server/lib/server/*` | Framed-protocol and dispatch glue. |
| Client contracts | `clients/lib/subsys/*` | Closed client contracts. No ext or platform deps. |
| Server extensions | `ext/server/{modules,drivers,providers,domain}` | Runtime-loaded server policy/mechanics. |
| Client extensions | `ext/client/{platforms,driver,module,capabilities}` | Platform runtimes and client open implementations. |
| Composition roots | `apps/*` | User-facing binaries and app libraries. |
| Tools | `tools/*` | Dev / test / perf only. Non-shipping. |
| Archive | `archive/*` | Non-normative heritage. Excluded from depgraph. |

The four `ext/client/<category>/` are: `platforms`, `driver`,
`module`, `capabilities`. Singular `driver` and `module`; plural
`capabilities`.

## 2. Allowed category edges

Missing edges are forbidden. Each row is "may depend on the listed
categories".

| From | May depend on |
|---|---|
| `arch/` | nothing (`core` only — `arch/` is the platform floor, §9/§10); selected `lib/*` only if explicitly listed in §6 sub-DAG |
| `lib/*` | `arch/`, peer/lower `lib/*` (per §6 sub-DAG) |
| `uapi/*` | `lib/*` only when the dep is target-neutral and ABI-safe; no server/client/ext/apps |
| `server/lib/subsys/*` | `uapi/*`, `lib/*`, `arch/`, allowed peer subsys edges |
| `server/lib/kernel/*` | `server/lib/subsys/*`, `uapi/*`, `lib/*`, `arch/` |
| `server/lib/server/*` | `server/lib/kernel/*`, `server/lib/subsys/*`, `uapi/protocol`, `lib/*` |
| `clients/lib/subsys/*` | `uapi/*`, `lib/*`, `arch/`, allowed client-subsys DAG edges |
| `ext/server/*` | `server/lib/subsys/*`, `uapi/*`, `lib/*`; named ext peers per manifest |
| `ext/client/platforms/*` | `clients/lib/subsys/*`, `uapi/*`, `lib/*` |
| `ext/client/driver/*` | `clients/lib/subsys/*`, `uapi/*`, `lib/*`, optional platform/capability contracts |
| `ext/client/module/*` | `clients/lib/subsys/*`, `uapi/*`, `lib/*`, capability contracts; no platform IO except via capabilities |
| `ext/client/capabilities/*` | `clients/lib/subsys/*`, `uapi/*`, `lib/*`, peer capability sub-DAG |
| `apps/server` | server runtime/contracts/foundation only; no concrete client extension crates |
| `apps/tui`, `apps/web` | selected `ext/client/platforms/<p>` plus client contracts/foundation |
| `apps/cli` | protocol/client CLI support; no server or extension implementation stack |
| `apps/reovim` | sibling app library targets and selected platform runtime under explicit `embedded-*` feature gates |
| `tools/*` | any in-repo crate if non-shipping and named in the probe catalog |

Every "may depend on" grant above is scoped to **in-repo crates**;
no row grants a third-party crate (`DAG5`, §9).

## 3. Forbidden category edges

| From | Must not depend on |
|---|---|
| **Every category** | third-party crates, in any dependency table (`DAG5`, §9) |
| Foundation | `server/*`, `clients/*`, `ext/*`, `apps/*`, `tools/*` |
| Server contracts | `ext/*`, `apps/*`, concrete Domain/provider modules |
| Server kernel | `ext/*`, `apps/*`, `clients/*`, server runtime, concrete Domain/provider modules |
| Client contracts | `ext/client/*`, `apps/*`, server kernel/runtime internals |
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
- the allocator,
- threads and sync primitives (mutex, rwlock, condvar equivalents),
- **ALL heap data structures** — the growable sequence, hash map,
  owned byte string, and shared-reference equivalents.

The project rule, stated plainly: *if we want a data structure or an
ABI-level facility, we create the DS and implement it at the `arch/`
level.* Crates above `arch/` consume `arch/`-provided data structures
only; no crate other than `arch/` declares foreign OS bindings (§9)
or heap primitives.

**`uapi/protocol` is zero-allocation.** The wire-protocol crate
(7.3 `SP15`) performs no allocation at all — callers provide every
buffer (`encode(&self, &mut [u8])`). It therefore needs NO `arch/`
data-structure edge; its only dependencies are `core` and `uapi`
siblings. This keeps the frozen ABI surface free of any platform
coupling.

**Illustrative-collection clause.** Where any chapter writes
`Vec<T>`, `HashMap<K, V>`, `String`, `VecDeque<T>`, or `Arc<T>` in an
illustrative struct listing, it denotes the `arch/`-provided heap DS
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
  `linux_x86_64`, `linux_aarch64`), selected by
  `#[cfg(all(target_os = "...", target_arch = "..."))]`; all backends
  expose identical floor function signatures — no traits, no vtables.
- All target-specific `asm!`/`naked_asm!` is confined to
  `arch/src/sys/<target>/` (raw syscall primitives and the fused clone
  trampoline) and to the cfg-gated `_start` entry arms in
  `arch/src/start.rs`. No inline asm may appear elsewhere in `arch/`.
- `errno.rs` and `wrap.rs` at the `sys/` level are Linux-kernel-ABI
  family code shared by Linux backends only. Non-Linux and freestanding
  backends provide their own wrap-level implementations behind the same
  floor signatures; nothing in the per-target structure assumes every
  backend routes through the shared Linux wrap layer.
- Targets with no registered backend fail at compile time with an
  explicit `compile_error!` fallback arm in `sys/mod.rs`.

The asm-confinement rule is enforced by a source-grep probe in
`lib/depgraph/tests/` (added #790).

## Open items

1. Foundation sub-DAG enumeration (§6) — deferred to the workspace
   scaffold, `TODO(#778)`.
2. Composition-root catalog format and probe wiring (§7) — deferred
   to the workspace scaffold, `TODO(#778)`.
3. Transitional allowlist current contents (§8) — resolved #782: the
   v0.16 rebuild starts greenfield; the allowlist starts **empty**
   (the #775 driver-display entry belonged to the archived v0.15
   tree).
4. `tools/*` forbidden-edge table — partially resolved #782: `DAG5`
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
| DAG5 | Fixture crate adds a registry dependency in each of the three dep tables in turn; expect **fail** for all three. Resolved-graph gate: lockfile/metadata package set is exactly the workspace-local set; a registry or git source → **fail**. |
| DAG6 | Three negative fixtures: a product crate missing `#![no_std]` at its root; a `use std::` in product code; a `use alloc::` in product code — each expects **fail**. Profile gate: workspace `panic` setting other than `"abort"` → **fail**. (Probe lands with the L10 enforcement flight; spec-first until then, like DAG5 pre-scaffold.) |
