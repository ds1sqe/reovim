# Package Manager Architecture

The reovim package manager (`#771`) is a six-crate stack plus a CLI
binary that takes a `pkg.toml` manifest, resolves its dependency
graph, writes a deterministic `pkg.lock`, materializes prebuilt
cdylibs into `$REOVIM_LIBRARY_ROOT`, audits the result, and gates
runtime loading behind lazy-load triggers.

For end-user workflow, see
[docs/user-guide/pkg.md](../user-guide/pkg.md). This document
covers internal architecture: the pipeline, the crate map, and the
layering invariants that future changes must preserve.

## Pipeline

```
manifest (pkg.toml) → resolver → lockfile (pkg.lock) → installer
                                                            ↓
                                                       inventory
                                                          ╱ ╲
                                                         ▼   ▼
                                                  lazyload   doctor
                                                  registry   (audit +
                                                     ↓        --fix)
                                                  trigger
                                                  dispatcher
```

- The **manifest** (`pkg.toml`) is the user-authored TOML source:
  `[package]` metadata, `[dependencies]`, `[lazy]` triggers.
- The **resolver** consumes the root manifest plus every reachable
  path-dep manifest, runs a DFS walk with semver constraint checks
  and conflict detection, and produces a `Resolved` set.
- The **lockfile** (`pkg.lock`) is the resolver's serialized output:
  one `[[package]]` block per resolved entry with target triple,
  optional SHA-256 digest, and optional lazy-load trigger string.
- The **installer** consumes a `Resolved` and materializes each
  package's cdylib (under `<pkg-dir>/dist/`) into the library root,
  runs a `dlopen` smoke probe, computes the SHA-256, and writes
  the **inventory** (the lockfile augmented with the digest and
  installed-path bookkeeping).
- The **inventory** feeds two independent consumers:
  - The **lazyload registry** + **trigger dispatcher** answer "which
    cdylibs should the runtime open right now?" given a
    `TriggerEvent` (domain / event / capability fired).
  - The **doctor** audits the inventory + on-disk state for the
    four fault classes: `unloadable`, `orphan`, `missing`, `drift`.
    Doctor has NO `pkg-lazyload` dependency — it lives inside
    `pkg-install` and inverts the installer's invariants directly.

## Crate map

| Crate | Owns |
|---|---|
| `lib/pkg-manifest` | TOML schema, `Manifest` / `Package` / `Dependency` types, `LazyTrigger` enum, `trigger_str` / `parse_trigger` encode/decode pair, `ManifestError`. |
| `lib/pkg-lockfile` | TOML envelope (`Lockfile`, `PackageLock`, `Source`). Stays free of `pkg-manifest` deps; `trigger` is stored as `Option<String>` so the encode/decode pair lives one layer up. |
| `lib/pkg-resolver` | Semver constraint solving, manifest loading, DFS graph walker, conflict detection, `Resolved.lazy` propagation, `into_lockfile` emission. |
| `lib/pkg-install` | cdylib discovery (`artifact`), `dlopen` probe (`probe`), SHA-256 hashing (`hash`), atomic placement under `kind_subdir` (`place`), inventory read/write/remove (`inventory`), and the `doctor` module that audits + repairs. |
| `lib/pkg-lazyload` | Runtime-side `LazyRegistry::from_lockfile`, `scan_eager` (filter cdylibs the OS scan should skip), and the trigger dispatcher (`names_to_load`, `load_triggered`, `TriggerEvent`). Deps: `pkg-manifest` + `pkg-lockfile` + `dylib-loader` + `thiserror` only — no `pkg-install`, no `pkg-resolver`. |
| `lib/dylib-loader` | Platform OS abstractions: `library_filename`, `cdylib_filename`, `pkg_name_from_cdylib_filename` (the inverse), `library_extension`, `Library::open`, `scan_paths`, the `Kind` enum with `FromStr`, and the `PathResolver` for `$REOVIM_*_PATH` resolution. Zero `reovim-*` production deps. |
| `tools/pkg` | The `pkg` CLI binary. Composition root: depends on every crate above and provides the seven subcommands (`lock`, `resolve`, `install`, `list`, `remove`, `trigger`, `doctor`). |

## Layering invariants

These five rules are locked across the chain. Any future refactor
that violates them is a regression:

1. **`trigger_str` / `parse_trigger` / `LazyTrigger` /
   `ManifestError::MalformedTrigger` live in `lib/pkg-manifest`.** All
   three consumers (resolver, install, lazyload) re-use them; no
   crate duplicates the encode/decode pair.

2. **The cdylib filename convention lives in `lib/dylib-loader`.**
   `cdylib_filename` (forward), `pkg_name_from_cdylib_filename`
   (inverse), `library_filename`, `library_extension`, and
   `Kind: FromStr` all live in `dylib-loader`. The crate has zero
   `reovim-*` production dependencies — adding one breaks the
   layering.

3. **`pkg-lazyload`'s dependency set is locked.** Only
   `pkg-manifest` + `pkg-lockfile` + `dylib-loader` + `thiserror`.
   Adding `pkg-install` or `pkg-resolver` here would tangle the
   runtime-side load path with the install-time pipeline.

4. **Doctor lives inside `pkg-install`, not as a separate crate.**
   Every fault class doctor reports inverts an install invariant
   (drift inverts `TamperedArtifact`; missing inverts the
   post-install state; orphan inverts the inventory-write step;
   unloadable inverts probe success). Co-locating them keeps the
   "doctor and installer must change together" obligation visible
   at the module level.

5. **`pkg-lockfile` stays free of `pkg-manifest` dependencies.**
   The lockfile tier is a pure TOML envelope; it stores the lazy
   trigger as `Option<String>` rather than `Option<LazyTrigger>` so
   the decode logic stays at the upper tier where `LazyTrigger`
   lives.

## Where things end up at runtime

A reovim binary (any of `apps/{server,tui,cli,reovim}/`) at startup:

1. Resolves the user's `pkg.toml` (or reads an existing `pkg.lock`).
2. Constructs a `LazyRegistry::from_lockfile(&lock)`.
3. Calls `reovim_pkg_lazyload::scan_eager(library_root, kind,
   &registry)` to enumerate the eager-only cdylibs and `dlopen`
   them via the existing driver/module loader subsys.
4. Holds the registry; when domains open / events fire / capabilities
   are requested, it calls `reovim_pkg_lazyload::load_triggered` to
   open the matching lazy cdylibs.

The user can run `pkg doctor` (or invoke `reovim_pkg_install::audit`
programmatically) at any time to audit the library-root state. The
`pkg trigger` CLI subcommand drives `load_triggered` directly,
useful for testing a `[lazy]` table or pre-warming a known trigger.

## Wave-3a: runtime loader integration (#771 Wave 3a)

Wave 3a wires the lockfile-driven lazy-load contract into the live
loaders on both sides of the kernel boundary.

### `pkg-runtime-loader` (lib/pkg-runtime-loader)

A small subsys-tier crate that the server and client driver-loader
share at runtime:

- `resolve_library_root()` — reads `$REOVIM_LIBRARY_ROOT` (the
  install-time variable set by `pkg install`).
- `lockfile_path(root)` — resolves the inventory `pkg.lock` location
  beside the cdylibs.
- `load_registry(root)` — reads `<root>/pkg.lock`, builds an
  `Arc<LazyRegistry>`, and returns the empty registry on a missing
  lockfile so a host that has not run `pkg install` keeps the
  pre-3a "load everything eagerly" behaviour.
- `package_name_for_path(path)` — recovers the package name from a
  `libreovim_pkg_<snake_name>.<ext>` filename, used by both the
  eager filter and the ABI-mismatch enrichment.
- `enrich_validation_error(path, err, wrap)` — generic helper that
  lifts a validation error into a side-specific
  `AbiMismatchAtPackage` variant when the cdylib filename matches
  the convention.

### Server: eager filter + on-event/on-domain dispatch

`server/lib/subsys/module-loader/` gained:

- `ModuleLoader::from_path_scan_filtered` — scan + dlopen path
  that consults `LazyRegistry::is_lazy(name)` and skips lazy
  entries before they are opened.
- `ModuleLoader::from_path_scan_filtered_diag` — same scan path
  but per-entry `IncompatibleVersion` errors are lifted into
  `LoadDiagnostic::AbiMismatchAtPackage` carrying the recovered
  package name. Bootstrap calls this variant so user-facing logs
  carry the package context.
- `LoaderHandle` — `Arc<Mutex<ModuleLoader>>` shared with the lazy
  dispatchers so they can dlopen one cdylib at a time after the
  bootstrap-owned `&mut ModuleLoader` is no longer reachable.

`apps/server/src/` gained two `LoaderHandle` consumers:

- `lazy_command_dispatch::LazyCommandDispatcher` — implements
  `CommandResolutionListener`; observes `CommandNameIndex::resolve`
  and dlopens any package whose lockfile `on-event = "<name>"`
  trigger matches the resolved command name.
- `lazy_domain_dispatch::LazyDomainDispatcher` — implements
  `DomainRegisterListener`; observes
  `SessionState::set_domain_driver` and dlopens any package whose
  `on-domain = "<name>"` trigger matches the registered domain.

Both dispatchers pre-populate a pending-set from the registry so
each lazy entry fires at most once per process lifetime; failed
loads are logged and dropped (fire-and-forget).

### Client: eager filter + on-capability dispatch

`clients/lib/subsys/driver-loader/` gained:

- `LoadedClientRender::from_path_scan_filtered` /
  `LoadedClientDebug::from_path_scan_filtered` — scan helpers that
  apply the same `LazyRegistry::is_lazy` filter before construction.
- `probe_from_path` — header-only ABI probe that distinguishes
  render-vs-debug vtables without constructing the driver.
- `lazy_capability::CapabilityLazyHook` — boot-time on-capability
  trigger. The platform runtime calls `dispatch_capability(name)`
  once per provided capability; each call probes any matching
  lazy cdylib and parks the loaded driver in an internal
  `DriverStore`.

`ext/client/platforms/tui/src/run.rs` calls
`load_packaged_drivers()` at startup and fans out
`dispatch_capability` over a static
`PROVIDED_CAPABILITY_NAMES = &["cell"]` list.

### ABI-mismatch diagnostics carry the package name

When a cdylib's filename matches the package-naming convention,
both sides surface the offending package name on ABI mismatch:

- Server: `LoadDiagnostic::AbiMismatchAtPackage { package, source }`
  wraps the inner `ModuleError::IncompatibleVersion`.
- Client: `LoadError::AbiMismatchAtPackage { package, source }`
  (single-load API) and
  `ScanEntryError::AbiMismatchAtPackage { package, source }`
  (scan API) wrap the inner
  `ValidationError::AbiVersionMismatch` /
  `ApiVersionIncompatible` / `SizeOfSelfMismatch`.

### Chain-acceptance proof

Two integration tests prove the chain end-to-end on each side
against real cdylibs and a synthetic lockfile (no `pkg install`
needed at test time):

- `apps/server/tests/wave_3a_chain_proof.rs` —
  `wave_3a_chain_proof_server_side`.
- `clients/lib/subsys/driver-loader/tests/wave_3a_chain_proof.rs`
  — `wave_3a_chain_proof_client_side`.

Each test exercises (a) lockfile read, (b) eager-only filter at
startup, (c-α) dispatcher composition fires the matching lazy
load, (d) ABI-mismatch enrichment carries the package name. The
client test also confirms `CapabilityLazyHook::dispatch_capability`
parks the loaded driver in `DriverStore`.

### Deferred to a follow-up flight

Wave 3a's chain proof is library-level, not bootstrap-level. The
server-side dispatchers ship as composable units and are unit-
and integration-tested directly, but bootstrap
(`apps/server/src/bootstrap.rs`) does not yet register
`LazyCommandDispatcher` on the `CommandNameIndex` or
`LazyDomainDispatcher` on the `Server`'s domain-listener path.
Similarly, the TUI's `PackagedDrivers` bundle is held alive on a
local binding but not yet threaded into the live render path.

The bootstrap-level hookup is staged for a follow-up flight
tracked at `tmp/deferral-draft-wave-3a-bootstrap-dispatcher-hookup.md`.
That flight will rework `apps/server/src/bootstrap.rs`'s
`ModuleLoader` ownership and the TUI render-path threading and
ship a bootstrap-level integration test on top of the chain proofs
landed by Wave 3a.

## Related documentation

- [docs/user-guide/pkg.md](../user-guide/pkg.md) — end-user workflow.
