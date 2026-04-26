# Package Manager (`pkg`)

The `pkg` CLI installs, lists, removes, locks, audits, and lazy-loads
reovim cdylib extensions (drivers and modules). It speaks a
cargo-like manifest + lockfile, a pacman-like install workflow, and a
lazy.nvim-like load-on-demand surface.

## Quick Start

A package is a directory with a `pkg.toml` manifest plus a `dist/`
holding the prebuilt cdylib. Install one:

```bash
$ tree my-extension
my-extension
├── pkg.toml
└── dist
    └── libreovim_pkg_my_extension.so
```

`my-extension/pkg.toml`:

```toml
[package]
name = "my-extension"
version = "1.0.0"
kind = "module"
reovim-version = "^0.15"
```

A consumer's user-config manifest declares it as a path dependency:

```toml
[package]
name = "my-config"
reovim-version = "^0.15"

[dependencies]
my-extension = { path = "./my-extension" }
```

Then:

```bash
export REOVIM_LIBRARY_ROOT=~/.local/share/reovim
pkg install
```

`pkg install` resolves the manifest, writes `pkg.lock`, smoke-tests
each cdylib via `dlopen`, hashes the bytes, and copies the artifact
into `$REOVIM_LIBRARY_ROOT/modules/` (or `driver/` for `kind =
"driver"`).

## Commands

### `pkg lock`

Resolve the manifest and write `pkg.lock` without touching the
library root. Useful for committing a deterministic lock to source
control before any install runs.

```bash
pkg lock                              # uses ./pkg.toml
pkg lock --manifest-dir path/to/proj  # explicit manifest dir
pkg lock --lockfile path/to/pkg.lock  # explicit lockfile output
```

Exit codes: `0` success, `1` resolution error.

### `pkg resolve`

Resolve the manifest and print the sorted package list to stdout
without writing anything to disk. Dry-run sibling of `pkg lock`.

```bash
pkg resolve                              # uses ./pkg.toml
pkg resolve --manifest-dir path/to/proj
```

Exit codes: `0` success, `1` resolution error.

### `pkg install`

Resolve, lock, and materialize every resolved cdylib into the
library root. Smoke-tests each cdylib via `dlopen` and records a
SHA-256 digest of the bytes; subsequent installs detect tampering
against the recorded digest.

```bash
pkg install --library-root $REOVIM_LIBRARY_ROOT
pkg install --manifest-dir path/to/proj --library-root /tmp/lib
pkg install --library-root /tmp/lib --lockfile /tmp/pkg.lock
```

Exit codes: `0` success, `1` resolve error / probe failure / tamper
detection / I/O error.

### `pkg list`

Print the installed inventory: one line per package with name,
version, kind, and absolute path. Reads `pkg.lock` and the library
root; never modifies state.

```bash
pkg list --library-root $REOVIM_LIBRARY_ROOT
pkg list --library-root /tmp/lib --lockfile /tmp/pkg.lock
```

Exit codes: `0` always (a missing lockfile prints nothing and exits
0).

### `pkg remove`

Drop a single inventory entry: deletes the cdylib from the library
root and removes the matching `[[package]]` block from the
lockfile.

```bash
pkg remove my-extension --library-root $REOVIM_LIBRARY_ROOT
```

Exit codes: `0` success, `1` package not in inventory or I/O error.

### `pkg trigger`

Open every cdylib whose lockfile trigger matches the given runtime
event. Used by reovim itself at runtime; also useful for manual
verification of a `[lazy]` table.

```bash
pkg trigger domain text       # fires every "on-domain = text" entry
pkg trigger event save        # fires every "on-event = save" entry
pkg trigger capability render # fires every "on-capability = render"
```

Exit codes: `0` success (including zero matches), `1` lockfile
parse / open error / missing cdylib for a triggered name.

### `pkg doctor`

Audit the library root against `pkg.lock` for four fault classes:

- **unloadable**: cdylib on disk fails to `dlopen`.
- **orphan**: file in `<library_root>/{driver,modules}/` with no
  matching inventory entry.
- **missing**: inventory entry whose cdylib is gone from disk.
- **drift**: on-disk SHA-256 disagrees with the digest recorded in
  the lockfile.

```bash
pkg doctor --library-root $REOVIM_LIBRARY_ROOT
pkg doctor --fix --library-root $REOVIM_LIBRARY_ROOT
```

`--fix` auto-remediates two of the four classes:

- **orphan** → delete the unowned file (path is printed for audit).
- **missing** → drop the stale entry from the lockfile.

Unloadable + drift findings are NEVER auto-fixed; they require user
judgement (re-install, re-build, or investigate). After `--fix` the
residual unloadable/drift findings still cause exit 1.

Exit codes: `0` no findings (or all fixed under `--fix`), `1`
findings remain or I/O error.

## Lazy loading

`[lazy]` in `pkg.toml` declares trigger events for dependencies;
matching cdylibs are NOT opened at startup. They open when the
event fires (a command name resolves, a domain registers, or a
capability is requested at platform startup).

Wave 3a wired the lockfile-driven lazy contract into the live
loaders. Authoring a `[lazy]` entry with one of three triggers
gates the cdylib open:

| Trigger | Fires when … | Side |
|---------|--------------|------|
| `on-event = "<name>"` | a server command resolves to that name (`:save`, `:write`, …) | server module |
| `on-domain = "<name>"` | a server session opens a domain with that name | server module |
| `on-capability = "<name>"` | the client platform's startup fan-out reaches that capability | client driver |

`eager = true` is the explicit opt-out; a package without `[lazy]`
defaults to eager. Drivers load on the client side; modules load
on the server side. The lockfile records the resolved trigger as
a flat string so the runtime tier can read it without depending
on the manifest crate.

```toml
[package]
name = "lazy-demo"
reovim-version = "^0.15"

[dependencies]
text-helper = { path = "./text-helper" }
save-hook = { path = "./save-hook" }
eager-mod = { path = "./eager-mod" }

[lazy]
text-helper = { on-domain = "text" }
save-hook = { on-event = "save" }
eager-mod = { eager = true }
```

After `pkg install`, the lockfile records each entry's trigger as a
flat string:

```toml
[[package]]
name = "text-helper"
trigger = "on-domain:text"
# …

[[package]]
name = "eager-mod"
trigger = "eager"
# …
```

A package without `[lazy]` defaults to `eager` — it loads at
startup. The runtime calls `pkg trigger domain text` (programmatic
equivalent: `reovim_pkg_lazyload::load_triggered`) when a text
domain opens; `text-helper`'s cdylib is `dlopen`ed at that point,
not before.

### Trigger semantics today vs the deferred bootstrap hookup

The runtime composition shipped in Wave 3a is library-level. The
dispatchers (`LazyCommandDispatcher`, `LazyDomainDispatcher`,
`CapabilityLazyHook`) are constructed and unit/integration tested
end-to-end against real cdylibs, but the *registration* of those
dispatchers on the live server's `CommandNameIndex` and
`Session`'s domain-listener path is staged for a follow-up flight
that reworks `apps/server/src/bootstrap.rs`'s `ModuleLoader`
ownership. On the client, the TUI platform runtime calls
`load_packaged_drivers()` at startup and fans out
`dispatch_capability` over `PROVIDED_CAPABILITY_NAMES = &["cell"]`,
but the resulting `DriverStore` is held on a local binding and not
yet threaded into the live render path.

In other words: today, a host that has run `pkg install` and
declared a `[lazy]` table will get the eager filter at startup
(the cdylib is correctly skipped) and the diagnostic surface on
ABI mismatch (the offending package name is logged). The lazy
*dispatch* fires from the manual `pkg trigger` CLI today; live
runtime fan-out lands with the deferred bootstrap-hookup flight.

### ABI-mismatch diagnostics

When a cdylib's `pkg.toml` was built against a different reovim
version than the host, the loader rejects it before any driver
function is dispatched. If the cdylib filename follows the
`libreovim_pkg_<name>.<ext>` convention, the diagnostic carries the
recovered package name so the user knows which entry needs a
re-install:

```
WARN packaged-module load failure
  error=ABI mismatch in package `wrong-api-module`: \
        module requires API 4294967295.0, kernel provides 1.0
```

The rendered text always contains the package name; depending on
which side surfaced the mismatch (server module vs. client driver)
the inner cause varies (`IncompatibleVersion` vs.
`AbiVersionMismatch` / `ApiVersionIncompatible` /
`SizeOfSelfMismatch`).

## Library-root layout

`pkg install` materializes each cdylib into one of two subdirectories
per its `kind`:

```
$REOVIM_LIBRARY_ROOT/
├── driver/
│   └── libreovim_pkg_<package_name>.so
├── modules/
│   └── libreovim_pkg_<other_name>.so
└── pkg.lock
```

Filenames follow the convention `libreovim_pkg_<snake_name>.<ext>`
(linux/macos) or `reovim_pkg_<snake_name>.dll` (windows), where
`<snake_name>` is the package name with hyphens replaced by
underscores.

`pkg.lock` (cargo-like, top-level `version` plus an array of
`[[package]]` entries) is the inventory: every installed cdylib has
exactly one entry recording its name, version, source path, kind,
SHA-256 digest, and (optional) lazy-load trigger.

## Troubleshooting

**"package `<name>` is not installed" on `pkg remove`** — the name
isn't in the lockfile inventory. Check `pkg list`; the package may
have been removed already or the lockfile path is wrong.

**"cdylib for `<name>` has changed since the last install"** — the
on-disk bytes drifted from the recorded digest. Either rebuild the
package's `dist/` artifact and re-run `pkg install`, or delete the
inventory entry with `pkg remove` and re-install.

**Doctor reports `unloadable`** — `dlopen` rejected the cdylib.
Common causes: a missing transitive `.so` dependency
(`ldd <path>`), a cross-compiled artifact that doesn't match the
host triple, or a corrupt build. Rebuild the source package.

**Doctor reports `drift`** — the bytes on disk don't match the
lockfile's recorded SHA-256. Same remediation as the install-time
tamper check above. `--fix` does NOT auto-remediate drift; it
requires user judgement on whether the new bytes are intentional.

**`pkg install` fails with "path-dep manifest `<name>` is missing
`[package].kind`"** — every path-dep manifest must declare its
`kind` (`driver` or `module`). Add `kind = "module"` (or `driver`)
to the dependency's `pkg.toml`.

## Related documentation

- [docs/architecture/pkg-manager.md](../architecture/pkg-manager.md)
  — internal pipeline + crate map.
- [CHANGELOG.md](../../CHANGELOG.md) `#771 Package Manager` entry.
