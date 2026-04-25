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

## Related documentation

- [docs/user-guide/pkg.md](../user-guide/pkg.md) — end-user workflow.
