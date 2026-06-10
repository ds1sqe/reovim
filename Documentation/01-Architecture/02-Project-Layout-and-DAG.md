# 1.2 — Project Layout and Dependency DAG

**Scope.** Source-tree categories for every workspace crate, the
allowed and forbidden dependency edges, the depgraph probe model, and
drift detection.

**Heritage.** v4 README §3; v3 `CLAUDE.md` Architecture; reviews from
`tmp/review/apps-layout-project-layout-depgraph-review.md`.

**Locked rules.** `DAG1`, `DAG2`, `DAG3`, `DAG4`, `DAG5`.

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
| `arch/` | nothing (`std` only — `arch/` is the FFI floor, §9); selected `lib/*` only if explicitly listed in §6 sub-DAG |
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
6. Reports a summary: per-category crate count, ambiguous-classification
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
expires = "v4.0-rc1"
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
> spec — the in-house framed wire protocol (7.3), `std`-realized
> concurrency contracts (2.1, 2.3), the raw-terminal layer (8.2),
> and hand-rolled dynamic loading (2.2) — each carry a heritage note
> at the owning chapter.

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
