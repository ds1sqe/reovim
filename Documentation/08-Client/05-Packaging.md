# 8.5 — Client Packaging

**Scope.** How client-side cdylibs are packaged, distributed, and
installed under the library root. Manifest kinds for client
artefacts.

**Heritage.** v4 README §0.4 (manifest-kind cleanup);
`07-Surfaces/01-Package-Manager.md`.

**Locked rules.** None new; references PM* and CL6/CL7.

---

## 1. Client manifest kinds

Reuse of the server manifest schema (7.1 §3, §4) with client-side
kinds:

| Kind | Path |
|---|---|
| `module-client` | `manifest/module/<name>.toml` (client-side flag) |
| `driver-client` | `manifest/driver/<name>.toml` |
| `capability-client` | `manifest/capability/<name>.toml` |
| `stream-scheme` | `manifest/stream/<name>.toml` (server-side; listed for kind-table completeness — 4.4 §7) |

The manifest carries a `target = "client"` field where ambiguous,
e.g. for a module that ships both `module-server` and
`module-client` cdylibs.

## 2. Capability manifest schema

```toml
# manifest/capability/<name>.toml
[package]
name           = "cell-view"
kind           = "capability-client"
schema_version = "1.0"
platforms      = ["tui"]
abi            = "1.0.0"
api            = "1.0.0"

[[vtable]]
kind   = "capability_cell_view"
symbol = "REOVIM_CAPABILITY_CELL_VIEW_VTABLE"

[depends]
capabilities = ["cell >= 1.0"]
```

## 3. Distribution shape

A client-package tarball contains:

```
<name>-<version>.tar.zst
├── package.toml             # top-level package descriptor
├── manifest/
│   ├── module/<name>.toml   (or driver/, capability/)
└── client/
    └── <kind>/<name>.so     (and .dylib, .dll variants per platform)
```

`pkg sync` extracts these into the library root layout (1.3 §4).

## 4. Multi-cdylib packages

A package may ship multiple cdylibs (e.g. a driver pair: render +
chrome). Each cdylib gets its own manifest entry; they share a
package version.

## 5. Cross-target packages

A package may include both server and client artefacts (e.g. the
`vim` package shipping `module-server/vim.so` and
`module-client/vim.so`). The package descriptor lists both.

## Open items

1. ~~Stream-scheme manifest kind~~ — resolved (4.4 §7):
   `stream-scheme`, its own `ManifestKind`, manifests at
   `manifest/stream/<name>.toml`.
2. Cross-platform bundles — single tarball for linux+macos+windows
   vs per-platform tarballs. Default: per-platform.
3. Whether capabilities need a `depends.platforms` field. Default:
   yes — the existing `platforms` field on `[package]` covers it.

## Conformance

| Behaviour | Fixture |
|---|---|
| Multi-cdylib unpack | Package with two cdylibs → both extracted, both manifested, lockfile entries created. |
| Cross-target package | Package with both server and client cdylibs → installed under their respective roots. |
