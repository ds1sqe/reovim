# 7.1 — Package Manager

**Scope.** `pkg sync`, lockfile, manifest schema, supply-chain
hardening, library-root path identity, project package overlays.

**Heritage.** `archive/docs/deployment/library-root.md`.

**Locked rules.** `PM1..PM9` carried (restated §2); `PM10..PM11`
new; references `SEC3..SEC4`.

---

## 1. Concepts

- **Package** — a unit shipping one or more cdylibs and their
  manifests. Distributed as a tarball / archive containing
  `package.toml`, the cdylib(s), and any companion files.
- **Lockfile** — `lockfile.toml` at the library root. SHA-256 of
  every shipped cdylib + manifest. Required for `pkg sync`.
- **Manifest** — per-artefact TOML file at `manifest/<kind>/<name>.toml`.
  Names symbol(s), kind, platforms, vtable list.
- **Library root** — `$ROOT/{module,driver,capability}/{server,client}/`
  install layout (1.3 §4).

## 2. Carried rules (PM1..PM9)

The rule bodies below are the normative texts.

> **PM1 — Lockfile integrity is SHA-256 in the first cut.**
> ed25519 sign-on-publish is added on top when a registry exists;
> SHA-256 stays as the corruption layer. The lockfile schema
> reserves signature slots that are absent until then. *Class*:
> tooling + runtime.

> **PM2 — Declared set and resolved set live apart.** The declared
> set (`pkgs.toml`) lives in the user config dir
> (`$XDG_CONFIG_HOME/reovim/`); the resolved set (`lockfile.toml`)
> and artefacts live at the library root. Declared-set is user
> config; resolved-set + artefacts are build output. *Class*:
> convention + tooling.

> **PM3 — `pkgs.toml` is user-editable.** `reovim pkg add` /
> `remove` are sugar over the same file. Comments and grouping are
> preserved on tool rewrites where possible (best-effort).
> *Class*: tooling.

> **PM4 — Global and per-project declared sets overlay-merge.**
> Global (`$XDG_CONFIG_HOME/reovim/pkgs.toml`) and per-project
> (`./reovim.toml`) sets are both supported; project entries
> overlay-merge with global (dotfiles model): project keys
> override matching global keys field-by-field; absent keys
> inherit. `reovim pkg list` shows the merged effective set with
> provenance. Project overlays never auto-execute (PM11).
> *Class*: tooling.

> **PM5 — Lazy-load placeholder replay is synchronous up to a
> bounded wait.** Default 150 ms, configured by
> `editor.host.[limits].replay-max-wait-ms` (a drain bound like
> every other entry in 2.2 §7; `[limits]` is host-class per 1.5
> §6). If exceeded, the kernel surfaces a transient
> `module loading: <name>…` notice on the active client; replay
> continues in the background; the user can cancel the queued
> replay (the load itself still completes). *Class*: runtime.

> **PM6 — Modules and drivers share one manifest schema.**
> Always-on drivers declare `trigger = ["startup"]`. The
> manifest's `kind` plus `[[vtable]]` entries (§3, §4; CL6, 6.2
> §7) select which vtable symbol(s) the loader reads. A failed
> init lands the cdylib in the `Failed` lifecycle state (2.2).
> There is no separate driver-manifest schema. *Class*: tooling +
> LF.

> **PM7 — Keybinding conflicts always warn.** Conflicts resolve by
> explicit `priority = N` (highest wins); default is lexicographic
> by module name. `pkg add` / `pkg sync` always emit a warning
> listing both modules and the resolved winner, even when
> `priority` resolves cleanly — silent overrides are not
> acceptable. (The kernel stays key-free; conflict detection is
> package/client policy, §10.) *Class*: tooling.

> **PM8 — `replay-max-wait` is two-tier.** Global
> `editor.host.[limits].replay-max-wait-ms` plus per-package
> `[pkg.<name>] replay-max-wait-ms` in `pkgs.toml`; the package
> value overrides global. `pkg sync` rejects per-package values
> outside `50..=60_000` ms with a named diagnostic and a DS12
> `kernel_log` entry. *Class*: tooling + runtime.

> **PM9 — The manifest declares per-cdylib serialization.**
> `[load] requires_serialization = false` (default). Cdylibs whose
> internal state is not safe to call concurrently from multiple
> workers opt in by setting `true`; the kernel then takes a
> per-cdylib mutex around every slot call (2.3). The flag is
> per-cdylib, not per-Domain or per-handler — once set, every slot
> serializes. Stream schemes MUST NOT opt in (S1, 4.4 §12).
> *Class*: LF + runtime.

**Reshape note (PM1..PM9).** Carried forward with this spec's vocabulary:
the replay bound's config home moved from the old
`[ui] replay_max_wait_ms` to `editor.host.[limits].replay-max-wait-ms`
(host-class, kebab per CFG10); the old `LoadError::ReplayMaxWaitOutOfRange`
named error is now a tooling diagnostic + DS12 event (`pkg sync` is
tooling, not an ABI surface; AB14 governs the ABI side); PM6's old
"AB11 kind field" reference became the manifest `kind` +
`[[vtable]]` mechanism (CL6, 6.2 §7). PM9
(`requires_serialization`) was briefly shadowed by a new rule
ID — the additions are renumbered PM10/PM11 and the original PM9 is
restored; CC13's per-cdylib serialization mutex is the kernel half
of PM9. Manifest field names stay snake_case (manifest schema);
config keys are kebab (CFG10).

## 3. Module manifest schema

```toml
# manifest/module/<name>.toml
[package]
name           = "vim"
kind           = "module-server"          # or "module-client"
schema_version = "1.0"
platforms      = ["linux", "macos"]       # or "*"
abi            = "1.0.0"
api            = "1.2.0"

[[vtable]]
kind   = "module"
symbol = "REOVIM_MODULE_VTABLE"

[[view_slot]]                              # zero or more (3.2)
kind = "vim.cursor-state"
abi  = "1.0"

[depends]                                  # optional cross-module deps
modules = ["theme >= 1.0"]
drivers = []
```

## 4. Driver manifest schema

```toml
# manifest/driver/<name>.toml
[package]
name           = "render-tui"
kind           = "driver-client"
sub_kind       = "render"                  # render | input | display | ...
schema_version = "1.0"
platforms      = ["tui"]                   # platform compat declared, NOT path
abi            = "1.0.0"
api            = "2.1.0"

[[vtable]]
kind   = "client_render"
symbol = "REOVIM_CLIENT_RENDER_DRIVER_VTABLE"
```

Multi-vtable cdylibs append additional `[[vtable]]` entries (8.3).

## 5. Lockfile schema

```toml
# $ROOT/lockfile.toml
[meta]
schema_version = "1.0"
created        = "2026-04-28T12:34:56Z"

[[entry]]
kind     = "module-server"
name     = "vim"
path     = "module/server/vim.so"
sha256   = "ab12...cd34"
manifest = "manifest/module/vim.toml"
manifest_sha256 = "ef56...78ab"

[[entry]]
kind     = "driver-client"
name     = "render-tui"
path     = "driver/client/render-tui.so"
sha256   = "..."
manifest = "manifest/driver/render-tui.toml"
manifest_sha256 = "..."
```

`[[entry]]` rows are **canonically ordered**: sorted
lexicographically by `(kind, name)`. `pkg sync` always emits this
order, so the lockfile is byte-reproducible from the same package
set and the registration order derived from it is deterministic
and install-history-independent. Dispatch tie-breaking depends on
this order (DT17, 4.1 §6).

## 6. `pkg sync`

```
1. read declared packages from pkgs.toml + ./reovim.toml
2. fetch artefacts (HTTPS / file / git ref) into staging
3. compute SHA-256 over each artefact + manifest
4. cross-check:
     - manifest kind/symbol matches embedded vtable header
     - manifest schema_version parses
     - artefact owner / perms / path identity (SEC3, §8)
5. write lockfile.toml; update library-root contents atomically
```

> **PM10 — `pkg sync` verifies lockfile + manifest + symbol +
> checksum + path identity in one transaction.** A change in any of
> these fails sync; partial state is rolled back.
>
> *Class*: runtime + tooling.

## 7. Project package overlays

```
./reovim.toml          # project-declared package set
```

Overlays are **declarative only**. They never auto-execute code.
Loading the overlay requires running `reovim pkg sync` (or
`reovim pkg verify` if the lockfile is up-to-date), which performs
all PM10 checks.

> **PM11 — Project package overlays never auto-execute.** Code from
> project overlays is loaded only after `pkg sync` / `pkg verify`
> has signed off. *Class*: spec-asserted + runtime.

## 8. Library-root path identity (SEC3, SEC4)

> **SEC3 — Library root path identity verified before `dlopen`.**
> Every path component from library root to artefact is checked
> for:
> - reject world-writable directories;
> - validate owner / perms;
> - resolve symlinks per SEC4;
> - bind verification to the resolved artefact identity (the same
>   inode that was checksummed is the one `dlopen`ed);
> - re-checksum the artefact at the resolved path immediately
>   before `dlopen`.
>
> Platforms where the equivalent path-identity guarantees cannot be
> established fail closed unless an explicit unsafe override is
> set and logged.
> *Class*: runtime.

> **SEC4 — Symlink and hardlink resolution policy.** `pkg sync`
> resolves symlinks during verification. A symlink that escapes the
> library root rejects the package. Hardlinks are tolerated within
> the library root; cross-root hardlinks reject.
> *Class*: runtime.

## 9. Unsigned / locally sourced cdylibs

Raw HTTPS / file / git cdylib sources (without an upstream signing
authority) are treated as unsigned native code. Loading requires:

- explicit `trust = "local"` declaration in the package's lockfile
  entry,
- visible flag in `inventory` showing the trust class,
- WARN log line at boot.

Without the explicit declaration, an unsigned cdylib is rejected.

## 10. Keybinding lazy-load triggers

- The kernel remains key-free.
- Keybinding conflicts and triggers are package/client/Domain
  policy.
- If the package manager records key-like triggers, the kernel sees
  only opaque `RawInputKind::Trigger` IDs — not parsed
  keymaps/modes.

## Open items

1. Trust-class enumeration for `lockfile.toml` entries
   (`trusted` / `local` / `verified` / ...).
2. Whether `pkg sync` in offline mode can satisfy verification with
   a cached fetch + checksum match (no network). Default: yes.
3. Lockfile schema migration policy.

## Conformance

| Rule | Fixture |
|---|---|
| PM1 | Flip one byte in a shipped artefact → checksum mismatch at verify. |
| PM4 | Project key overrides one field of a global entry; `pkg list` shows provenance for both. |
| PM5 | Slow-loading module exceeds replay bound → transient notice; replay completes in background; cancel drops the queue, load still completes. |
| PM7 | Two modules bind the same key without priority → lexicographic winner; warning lists both. |
| PM8 | Per-package `replay-max-wait-ms = 10` → `pkg sync` rejects (below 50) with diagnostic + DS12. |
| PM9 | Serialized cdylib: two concurrent slot calls observed sequential; stream-scheme manifest with the flag → `Conflict` at init (S1). |
| PM10 | Tampered cdylib post-sync → next `pkg verify` fails; refuses load. |
| PM11 | Project overlay with new package; bare `reovim` does not load it; `pkg sync` then loads it. |
| SEC3 | World-writable parent dir → `pkg sync` rejects. |
| SEC4 | Symlink to /etc/passwd inside library root → reject. |
