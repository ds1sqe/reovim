# 1.3 — Apps and Invocation

**Scope.** User-facing binaries, their roles, embedded vs subprocess
composition, the runtime library-root layout, and the invariants the
launcher imposes on transport semantics.

**Heritage.** `archive/docs/deployment/library-root.md`.

**Locked rules.** `AL1..AL10` carried (restated §6), `AL-INPROC` and
`AL11` (§8) new.

---

## 1. Binaries

| Binary | Role |
|---|---|
| `reovim-server` | Server runtime. Owns the editor core and server registries. |
| `reovim-tui` | TUI client runtime. |
| `reovim-cli` | One-shot CLI client / control surface. |
| `reovim-web` | Web client runtime / SSR entry. |
| `reovim` | Top-level launcher. Embedded-default plus subprocess/external modes. |
| `reovim-os` | RTOS image composition root. Builds bootable images and per-profile app bundles; not a hosted process binary. |

`reovim-cli ...` is the canonical CLI binary. `reovim cli ...`,
`reovim pkg ...`, and similar top-level forms MAY exist as launcher
aliases, but they MUST delegate to the CLI path without importing
plugin/package implementation logic into the launcher hot path.

`reovim-os` is the planned `apps/os/` composition root for RTOS-itself
distributions. It selects the floor/provider, root-daemon boot profile, and
per-arch bundle (shell-only, recovery, appliance), then emits a bootable image
such as `kernel8.img` for Raspberry Pi 4 or a Multiboot/q35 image for x86_64.
It is not a user-invoked hosted process and does not move system-kernel policy
into `apps/`; `apps/os` only wires the image, providers, and payload registry.

## 2. Composition modes

### 2.1 Embedded default

`reovim` (no flags) launches a single-process composition: the
server library and the selected client platform runtime are linked
into the same process. Framed-protocol encode/decode is replaced by
an in-memory transport adapter.

Cargo wiring:
- `apps/reovim` library-target deps on sibling app library targets.
- Feature-gated: `embedded-tui`, `embedded-cli`, `embedded-web`.
- The launcher's runtime path branches on the gate; the unselected
  sibling apps are not linked.

### 2.2 Subprocess

`reovim --subprocess` (or default if embedded-* features absent)
forks `reovim-server` and the selected client binary, connecting
them over the configured transport from `editor.host` config.

### 2.3 External

`reovim` connects to an externally launched server. No fork; only
client and transport logic.

## 3. AL-INPROC

> **AL-INPROC — Embedded mode preserves protocol semantics.**
> Embedded server/client mode uses the same session, attach-stream,
> capability-negotiation, backpressure, auth-decision, and
> debug-routing semantics as subprocess mode. It MAY replace
> framed-protocol encode/decode with an in-memory transport adapter only.
>
> *Class*: spec-asserted, runtime tests on the in-memory adapter.

This rule blocks shortcuts where the embedded path bypasses
capability negotiation or auth checks because "we're in the same
process".

## 4. Library-root layout

```
$ROOT/
├── module/
│   ├── server/<name>.(so|dylib|dll)
│   └── client/<name>.(so|dylib|dll)
├── driver/
│   ├── server/<name>.(so|dylib|dll)
│   └── client/<name>.(so|dylib|dll)
├── capability/
│   └── client/<name>.(so|dylib|dll)
├── manifest/
│   ├── module/<name>.toml
│   ├── driver/<name>.toml
│   └── capability/<name>.toml
└── lockfile.toml
```

Platform compatibility is **manifest metadata**, not a path
component. Example manifest:

```toml
[package]
name = "tui-render"
kind = "driver-client"
platforms = ["tui"]
schema_version = "1.0"

[[vtable]]
kind   = "client_render"
symbol = "REOVIM_CLIENT_RENDER_DRIVER_VTABLE"
```

Multi-vtable manifests append additional `[[vtable]]` entries; see
`08-Client/03-Drivers-Modules-Capabilities.md`.

## 5. Library-root resolution

Search-path priority for runtime discovery:

1. `--library-root <path>` CLI flag.
2. `$REOVIM_LIBRARY_ROOT` env var.
3. `editor.host` config field `[runtime].library-root` (if set).
4. Platform default:
   - Linux: `$XDG_DATA_HOME/reovim/library/` then `/usr/lib/reovim/library/`.
   - macOS: `~/Library/Application Support/Reovim/library/` then `/Library/Application Support/Reovim/library/`.
   - Windows: `%LOCALAPPDATA%\Reovim\library\` then `%PROGRAMDATA%\Reovim\library\`.

Resolution stops at the first existing directory that contains a
valid `lockfile.toml`.

## 6. Carried rules (AL1..AL10)

The rule bodies below are the normative texts.

> **AL1 — Five separate bins.** `reovim-server`, `reovim-tui`,
> `reovim-cli`, `reovim-web`, and the `reovim` launcher are
> separate binaries (§1), each its own composition root under
> `apps/`. *Class*: kernel-enforced (compile — workspace layout +
> depgraph).

> **AL1-RTOS — RTOS image is an app composition root.** Bootable RTOS images
> are composed under `apps/os/`, not under `arch/tests/fixtures`. The arch
> fixture tree may keep CI proof images, but official per-arch image packaging
> and app bundle selection belong to the `apps/os` composition root. *Class*:
> convention + depgraph/manifest scan.

> **AL2 — Bare `reovim` defaults to TUI.** Overridable via the
> `[default].client` field of the `editor.host` config
> participation (it selects the transport profile, hence
> host-class; 1.5 §6). *Class*: spec-asserted.

> **AL3 — Platform-native subprocess transport.** Unix socket on
> Unix-likes; named pipe on Windows. *Class*: spec-asserted.

> **AL4 — System-wide install root is `/usr/lib/reovim/`** (Linux;
> platform equivalents per §5). System-vs-user files are config
> layer 2 vs layer 3 in the 1.5 layer stack; the library root
> search order is §5. *Class*: convention.

> **AL5 — The launcher has no plugin awareness.** No manifest
> parsing, no package logic, no module/driver imports in the
> launcher hot path (§1). *Class*: kernel-enforced (compile —
> depgraph).

> **AL6 — Per-bin standalone.** Every bin boots and serves alone;
> enforced by depgraph probes plus a per-bin smoke test. *Class*:
> kernel-enforced (compile + runtime).

> **AL7 — Editor-mode argument contract.** The launcher carries
> positional arguments as a raw `Vec<String>` — no suffix parsing.
> It fills the `$EDITOR` role with vim-compatible exit codes and
> keeps no persistent state across editor-mode invocations.
> *Class*: spec-asserted.

> **AL8 — Privilege-mismatch plugin isolation.** Editor-mode reads
> the *invoking* user's `pkgs.toml` even when running under a
> different UID (the sudoedit shape). *Class*: spec-asserted.

> **AL9 (reshaped) — The editor core's own configuration is two config
> participations.** The prior flat `config.toml` tables (`[ui]`,
> `[limits]`, …) are superseded by the `editor.host` (privileged)
> and `editor.shell` (user-class) participations in the 1.5 config
> service, schema'd and layered per CFG1..CFG10. Out-of-range host
> values still fail boot (CFG9). Body: `05-Configuration.md` §6.
> *Class*: spec-asserted.

> **AL10 — Domain-pluggable positional-arg dispatch.** The
> launcher passes each raw arg to
> `EditorCore::resolve_positional_arg(raw, cwd)`; the editor core walks
> Domains' `OnPositionalArg` handlers in `(band, priority)` order
> with the DT17 tie-break; the first `PositionalClaim` wins. If
> every handler returns `Pass`, the generic fallback is
> `attach_buffer(PathBuf::from(raw))` with no positional payload.
> The launcher MUST NOT parse `path:LINE:COL` or any
> Domain-specific suffix. *Class*: spec-asserted.

**Reshape note (AL1..AL10).** Carried forward with this spec's vocabulary.
Config homes moved into the 1.5 layer stack: AL2's
`[default].client` is `editor.host` (transport-selecting), AL4's
system-vs-user split became layers 2/3, and AL9 is the one body
reshape — flat `config.toml` tables superseded by the schema'd
participations (CFG namespace). AL10 gains the DT17 tie-break for
its handler walk. AL1/AL3/AL5..AL8 are unchanged.

## 7. Cargo install

`cargo install reovim` installs the launcher and the TUI sibling
library target. The default `embedded-tui` feature is enabled,
giving an OOB single-process experience. Modules and drivers are
discovered from the workspace target directory or
`$XDG_DATA_HOME/reovim/library/` per §5.

## 8. Logging flags (AL11)

> **AL11 — Logging flags are layer-6 sugar.** Every bin accepts:
>
> - `-v` ⇒ `--set editor.host.log.level=debug`,
> - `-vv` ⇒ `--set editor.host.log.level=trace`,
> - `--log <PATH>` ⇒ `--set editor.host.log.path=<PATH>`.
>
> The flags desugar to layer-6 CLI config (1.5 §3) before the layer
> stack merges; they introduce no mechanism beside the 9.5 sink keys
> and obey normal precedence (a layer-7 force-override still wins).
> *Class*: spec-asserted + conformance.

## Open items

1. ~~Detach lifecycle ordering~~ — resolved (2.2 §8, LF12,);
   identical sequence in embedded and subprocess modes
   (AL-INPROC requires it).
2. Which `editor.host` fields are required at boot before any
   participant loads (forwarded to `01-Architecture/05-Configuration.md` §9).
3. `$REOVIM_LIBRARY_ROOT` precedence vs CLI flag — current spec
   says CLI wins; confirm against §5.

## Conformance

| Rule | Fixture |
|---|---|
| AL-INPROC | Embedded boot uses the in-memory adapter; capability negotiation runs; missing capability rejects the same way subprocess rejects. |
| AL1/AL6 | Depgraph probe: five composition roots; each bin smoke-boots standalone. |
| AL1-RTOS | Manifest scan: `apps/os` owns RTOS image profiles and no new official image path lands under `arch/tests/fixtures`. |
| AL2 | No-flag launch with `[default].client = "cli"` → CLI client selected. |
| AL5 | Depgraph probe: launcher crate has no package/module/driver deps; grep for manifest parsing in launcher. |
| AL7 | `reovim file.txt:12:3` opens a buffer named `file.txt:12:3` unless a Domain claims it (AL10); exit codes match vim contract. |
| AL8 | Editor-mode under `sudo -e` shape reads invoking user's `pkgs.toml` (fixture with two UIDs). |
| AL9 | Out-of-range `editor.host` value → boot fails with named diagnostic (CFG9). |
| AL10 | Domain claims `path:LINE:COL`; with no claiming Domain, generic attach; launcher source contains no suffix parser (CI grep). |
| AL11 | `-v` boots with sink level debug; `--log <PATH>` redirects the sink; `--force-set editor.host.log.level=info` beats `-vv` (layer 7 > 6). |
