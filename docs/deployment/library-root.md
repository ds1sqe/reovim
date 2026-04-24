# Library Root — Server Module Install Layout

After #769 Phase 3, `reovim-server` no longer statically links its builtin
modules. All 55 builtin modules plus any third-party modules are discovered
at runtime as `cdylib` shared libraries (`.so` on Linux, `.dylib` on macOS,
`.dll` on Windows). The server scans a fixed list of search paths, matches
discovered files against an embedded manifest, and loads them in dependency
order.

This document covers:

1. Search-path priority
2. Manifest contract
3. Install layouts (dev, system, user overlay)
4. Override knobs

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
