# Library Root — Server Module + Driver Install Layout

After #769 Phase 3, `reovim-server` no longer statically links its builtin
modules. All 55 builtin modules plus any third-party modules are discovered
at runtime as `cdylib` shared libraries (`.so` on Linux, `.dylib` on macOS,
`.dll` on Windows). The server scans a fixed list of search paths, matches
discovered files against an embedded manifest, and loads them in dependency
order.

After #769 Phase 4 scaffolding lands, the same library-root convention also
covers server-side **drivers**. After #769 Phase 5, it also covers
client-side **drivers**. Both migrated driver sets are currently empty
(see [Drivers](#drivers) below); server driver migrations land at follow-up
issue #774, and client driver migrations land at #753 client-foundation
resumption (pending #774 trait redesigns). The layout described here is the
target operators should expect once those issues populate it.

This document covers:

1. Search-path priority (modules)
2. Manifest contract (modules)
3. Install layouts (dev, system, user overlay) — modules
4. Override knobs — modules
5. Drivers — server-side and client-side layout, search paths, currently-empty disclaimers

See also [Server Mode](../user-guide/server-mode.md) and
[Server Overview](../architecture/server/overview.md).

## Search-path priority

`ModuleLoader::discover()` consults paths in this order, loading the first
match for each module id:

1. `$REOVIM_MODULE_PATH` (colon-separated on Unix, semicolon-separated on Windows)
2. `$XDG_DATA_HOME/reovim/modules/` (user install; defaults to
   `~/.local/share/reovim/modules/` when `XDG_DATA_HOME` is unset)
3. `/usr/local/lib/reovim/modules/` (locally-compiled system install)
4. `/usr/lib/reovim/modules/` (distribution package install)

A module found at a higher-priority path masks the same module id at a
lower-priority path. This lets a developer override a system-installed
module by dropping a freshly-built `.so` into their user directory without
disturbing the package install.

Source of truth: `server/lib/subsys/module-loader/src/discovery.rs`
(`default_search_paths()`).

## Manifest contract

`apps/server/builtins.toml` is embedded into `reovim-server` at build time
via `include_str!`. It lists every builtin module id, its init tier, and
the shared-library filename (without the `lib` prefix or platform
extension):

```toml
[[modules]]
id = "undo"
tier = 1
library = "reovim_module_undo"
```

At runtime the server:

1. Parses the embedded manifest to get the canonical id → library mapping
   and the tier-ordered list of builtins.
2. Runs `ModuleLoader::discover()` over the search paths.
3. For each manifest entry, loads the first `.so` file on the search path
   whose library name matches.
4. Merges in any additional `.so` files that are NOT in the manifest as
   third-party modules, placed after the builtins in the dependency graph.
5. Resolves dependencies and inits each module in order.

The manifest is the canonical **enumeration** of builtins; it does not
encode dependency edges. Each module's dependencies are reported by the
module itself during load (via its FFI descriptor).

Master-plan note: the master plan originally listed "`builtins.toml`
manifest deleted" among Phase 3 deliverables. That deliverable was
reinterpreted in Phase 3.A — the **reader code** for `static-modules` was
deleted, but the manifest itself is retained as the single source of truth
for the cdylib scan path. Without the manifest, the loader cannot
distinguish builtins from third-party modules or apply tier ordering.

## Install layouts

### 1. Developer workspace (`cargo build`)

After `cargo build --workspace`, module `.so` files land in the workspace
target directory:

```
target/debug/
├── reovim-server              # the server bin
├── libreovim_module_undo.so
├── libreovim_module_vim.so
├── libreovim_content_codec_utf8.so
├── libreovim_picker_files.so
└── ...
```

To run the server out of this build against these modules:

```bash
REOVIM_MODULE_PATH="$PWD/target/debug" ./target/debug/reovim-server --grpc 12540
```

This is the same pattern `tools/testing/src/harness.rs` uses for
integration tests (`workspace_module_dir()` at harness.rs:72).

### 2. System install (`/usr/lib/reovim/modules/`)

A distribution package places the server bin in `/usr/bin/reovim-server`
and the modules in `/usr/lib/reovim/modules/`:

```
/usr/bin/
└── reovim-server

/usr/lib/reovim/modules/
├── libreovim_module_undo.so
├── libreovim_module_vim.so
└── ...
```

No env vars are needed — the loader's default search path already covers
`/usr/lib/reovim/modules/`. Running `reovim-server --grpc 12540` from any
working directory will find and load the modules.

### 3. User overlay (`$XDG_DATA_HOME/reovim/modules/`)

A user who wants to test a locally-patched module without disturbing the
system install drops the patched `.so` into their user directory:

```
$XDG_DATA_HOME/reovim/modules/
└── libreovim_module_vim.so    # patched local build
```

The user install is **first** in the default search path (after
`$REOVIM_MODULE_PATH`), so the patched `libreovim_module_vim.so` wins over
the `/usr/lib/reovim/modules/libreovim_module_vim.so` of the system
package. All other modules still load from the system directory — the
overlay is per-file, not whole-directory.

## Override knobs

### `REOVIM_MODULE_PATH`

Adds high-priority search directories ahead of the XDG and system paths.
Use this for CI, testing against specific build outputs, or development
overlays.

```bash
export REOVIM_MODULE_PATH="/opt/reovim-plugins:$HOME/dev/reovim/target/debug"
reovim-server --grpc 12540
```

Accepts multiple colon-separated paths on Unix, semicolon-separated on
Windows.

### Per-module enable/disable (`ModulesConfig`)

User-level module enable/disable lives in the user's module-config TOML
(resolved via `ConfigPaths::config_dir()`), not in the manifest:

```toml
[modules.vim]
enabled = true

[modules.emacs]
enabled = false
```

The server reads this config during bootstrap and filters the init list
accordingly. Disabled modules are not loaded into memory — their `.so`
files on disk are left untouched.

## `cargo install reovim` note

Installing the reovim workspace via `cargo install` places the `reovim`
launcher and `reovim-server` bins into the user's cargo bin directory
(typically `~/.cargo/bin/`), but **does not install module `.so` files**.
Cargo does not ship `cdylib` artifacts through `cargo install` — the
modules must be built separately and placed on one of the search paths
above.

A packaging workflow that produces a complete install tree (bins + modules
in a versioned directory) is tracked under #769 Phase 6 as separate
deployment tooling. For now, after `cargo install reovim`, running the
server binary with no `.so` files on any search path will succeed at
startup but produce a minimal shell with no keybindings, syntax
highlighting, or other module-supplied features.

To restore a working install from a workspace build after `cargo install`:

```bash
mkdir -p ~/.local/share/reovim/modules
cp target/debug/libreovim_*.so ~/.local/share/reovim/modules/
```

## Drivers

After #769 Phase 4 and Phase 5, the library root also hosts **driver**
shared libraries for both server-side and client-side drivers. Drivers are
runtime-loaded cdylibs that implement subsys trait contracts (e.g. the
per-trait vtable exported as `REOVIM_<KIND>_DRIVER_VTABLE`).

`scripts/build-driver.sh` is the canonical build path for both tiers
(sibling to `scripts/build-module.sh`):

```bash
./scripts/build-driver.sh --all          # Build all migrated drivers (release)
./scripts/build-driver.sh --all --debug  # Build all migrated drivers (debug)
./scripts/build-driver.sh <name>         # Build single migrated driver
```

The script maintains two arrays (`MIGRATED_SERVER_DRIVERS` and
`MIGRATED_CLIENT_DRIVERS`) and stages `.so` artifacts to the appropriate
tier directory. Both arrays are currently empty; the script handles the
empty-set case by logging a notice and exiting 0.

### Server-side drivers

After #769 Phase 4, the library root hosts server-side driver shared
libraries under `$ROOT/driver/server/`.

> **Status (v0.15.0-dev):** The server-driver migrated set is currently
> **empty**. Migrated server drivers land under follow-up #774. Operators
> do not need to install any server driver `.so` files for the current
> release.

As of #769 Phase 4, the scaffolding ships:

- The `lib/depgraph/tests/no_static_drivers_feature.rs` ratchet probe
  (asserts no workspace `Cargo.toml` defines a `static-drivers` feature,
  preserving the master plan's L1 invariant).
- The `scripts/build-driver.sh` build pipeline (`MIGRATED_SERVER_DRIVERS`
  array; handles the empty set gracefully).
- This documentation describing the future layout.

Server driver migrations land at follow-up issue **#774**
(`feat(server-drivers): redesign net-grpc / command / text-session /
text-input / text-syntax trait surfaces for FFI-routability`).
**Operators do not need to install any server driver `.so` files for the
v0.15.0-dev release**; the existing in-process driver implementations
continue to work via compile-time linkage in `apps/server`. The
`# trait-redesign deferred to #774` rationale comments on the
`apps/server/Cargo.toml` driver deps document this.

#### Search-path priority (target layout, post-#774)

Per the master plan §"Loader + library-root discovery", server driver
discovery will use these search paths in priority order (first-match wins):

1. `$REOVIM_DRIVER_PATH` (env var, colon-separated on Unix,
   semicolon-separated on Windows)
2. `$REOVIM_LIBRARY_ROOT/driver/server/`
3. `$XDG_DATA_HOME/reovim/driver/server/` (defaults to
   `~/.local/share/reovim/driver/server/` when `XDG_DATA_HOME` is unset)
4. `/usr/local/lib/reovim/driver/server/` (locally-compiled system
   install)
5. `/usr/lib/reovim/driver/server/` (distribution package install)

Confirmation against `lib/dylib-loader/`'s actual rule lands when #774
populates the migrated set.

#### Layout under the library root

```
$REOVIM_LIBRARY_ROOT/
├── modules/
│   ├── server/...
│   └── client/...
└── driver/
    ├── server/...      # ← server-side drivers (this subsection)
    └── client/...      # ← client-side drivers (next subsection)
```

#### Builtin-vs-external priority

The same priority rule that applies to modules applies to drivers: a
driver found at a higher-priority path masks the same vtable kind at a
lower-priority path. This lets a developer override a system-installed
driver by dropping a freshly-built `.so` into their user directory
without disturbing the package install.

#### CLI flags

Once #774 lands a migrated server driver, the per-bin CLI gains
driver-specific flags mirroring the existing module flags:

- `--driver <PATH>` / `-d <PATH>` — additional driver directories
  (high-priority, prepended to the search list). Multiple values
  allowed.

These flags are **not introduced** until #774 lands a driver; the bin
has no use for them in the empty-set state.

### Client-side drivers

After #769 Phase 5, the library root also hosts client-side driver
shared libraries under `$ROOT/driver/client/`.

> **Status (v0.15.0-dev):** The client-driver migrated set is currently
> **empty**. Client driver trait-redesign work for `ChromeSurface` and
> `DisplayDriver` is tracked at follow-up #774. The `ClientRender`
> concrete implementor migration is deferred to #753 client-foundation
> resumption (pending #774 trait redesigns). Operators do not need to
> install any client driver `.so` files for the current release.

As of #769 Phase 5, the scaffolding ships:

- The `scripts/build-driver.sh` build pipeline (`MIGRATED_CLIENT_DRIVERS`
  array; handles the empty set gracefully; stages to `driver/client/`).
- The `uapi/driver-macros/src/client_render.rs` macro and the companion
  `ClientRenderVTable` + `RenderTargetVTable` types at
  `clients/lib/subsys/render/src/abi.rs` (Phase 0 deliverables; the
  foundation is operational).
- The `clients/tui/Cargo.toml` driver deps carry
  `# trait-redesign deferred to #774` rationale comments documenting the
  pending migrations.
- This documentation describing the future layout.

Client driver migrations land at:

- **#774** — `ChromeSurface`-family and `DisplayDriver` trait redesigns
  for FFI-routability, then cdylib migration.
- **#753 client-foundation resumption** — `ClientRender` concrete
  implementor landing once #753 Phase E's v7 flat-category layout is
  established.

#### Search-path priority (TBD — pending first migration)

Client driver discovery will use a parallel set of search paths under the
`client/` suffix. The exact `$REOVIM_CLIENT_DRIVER_PATH` env-var name and
priority order mirror the server-side pattern and will be confirmed when
#774 or #753 lands the first client driver migration. Expected form:

1. `$REOVIM_CLIENT_DRIVER_PATH` (env var, colon-separated on Unix)
2. `$REOVIM_LIBRARY_ROOT/driver/client/`
3. `$XDG_DATA_HOME/reovim/driver/client/` (defaults to
   `~/.local/share/reovim/driver/client/`)
4. `/usr/local/lib/reovim/driver/client/`
5. `/usr/lib/reovim/driver/client/`

Confirmation against `lib/dylib-loader/`'s actual rule lands when #774
or #753 populates the migrated set.

#### Layout under the library root

```
$REOVIM_LIBRARY_ROOT/
└── driver/
    ├── server/...      # server-side drivers
    └── client/...      # ← client-side drivers (this subsection)
```

#### CLI flags

Not introduced until a client driver migrates. The expected interface
mirrors the server-side `--driver` / `-d` flags; operators can plan their
packaging story on that basis.

#### Build pipeline

`scripts/build-driver.sh` handles client-driver `.so` staging via its
`MIGRATED_CLIENT_DRIVERS` array. When #774 or #753 lands a client driver,
adding the crate short-name to that array is the only edit needed. The
script stages the `.so` to `target/<profile>/lib/reovim/driver/client/`,
mirrors the server-side `nm`-based vtable symbol audit, and handles the
empty-set case by logging a notice and exiting 0.
