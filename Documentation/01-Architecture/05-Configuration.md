# 1.5 — Configuration

**Scope.** The config service: participants, layer stack, trust
classes, schemas, threat model, redaction, project root resolver,
env-var encoding. The ABI surface (`ConfigSlice`) lives in
`06-ABI/04-Config-Slice.md`; the user CLI lives in
`07-Surfaces/04-Config-CLI.md`.

**Heritage.** the q3-config-system RFC v2 (folded).

**Locked rules.** `CFG1..CFG10`.

---

## 1. Mental model

The editor core owns the product **config service**. It holds no opinion about what
any participant's config means. It owns:

- discovering participants (artefacts that declare a config schema),
- materialising defaults from the schema,
- collecting layered overrides,
- merging the layers per a single locked precedence,
- validating the merged result against the schema,
- handing each participant its own `ConfigSlice`,
- exposing introspection (`reovim config dump`) with per-field
  source and per-field redaction.

Mechanism vs policy: the editor core owns the machinery. Each participant
owns the policy.

## 2. Participants

| Kind | Namespace | Loaded at | Trust default |
|---|---|---|---|
| editor/host | top-level `[transport]`, `[limits]`, `[overrides]` | `EditorInit::boot` (L21 stage 2) | host (no project overlay) |
| editor/shell | top-level `[ui]`, `[editor]`, `[files]`, `[default]` | `EditorInit::boot` (L21 stage 2) | shell (project overlay OK) |
| module | `module.<name>` | cdylib load (PM3) | per-section in schema |
| driver | `driver.<kind>.<name>` | cdylib load (driver-load) | per-section in schema |
| package set | `pkgs` | PM4 path | participant-owned |

`ParticipantId` is the namespace string. Names match `[a-z0-9-]+`;
namespaces are dot-separated sequences.

## 3. Layer stack

For every participant, layers merge last-wins per field:

```
1. defaults             (materialised from schema)
2. system file          (/etc/reovim/...)
3. user file            ($XDG_CONFIG_HOME/reovim/...)
4. project file         (./.reovim/... — only fields where project_overlay=true)
5. environment          (REOVIMCFG_<encoded>__<encoded>; see §10)
6. CLI flags            (--set <ns>.<field>=<value>)
7. force-override       (--force-set, --force-config; see §7)
```

Order is total and global. No participant may request a different
order for its own surface.

## 4. Trust classes

Trust is per-field (declared in the schema):

| Class | Project overlay | Env / CLI | Force | Examples |
|---|---|---|---|---|
| `host` | NO | YES, boot-only | YES, logged | transport, library roots, registries, credentials, safety overrides |
| `shell` | YES | YES | YES, logged | UI/editor preferences, scrollback, safe keybinding policy |
| `module-private` | YES if declared | NO via regular env/CLI | YES, logged | self-tuning |

## 5. Threat model — what MUST be host

A field MUST be host-class if it can:

- execute any command, hook, shell string, or external program;
- alter module load paths, library search paths, plugin search paths;
- change network registries, mirror URLs, fetch endpoints;
- hold or redirect authentication material (credentials, tokens, certs);
- change the transport surface (port, socket path, listener address);
- disable editor-core safety knobs (`[overrides]`).

A schema that marks any of the above as `shell` or `module-private`
fails schema validation at load with `IllegalTrustClass`.

Conformance fixture: a schema with a "command-string" field declared
`trust = "shell"` MUST fail load.

## 6. EditorCore host vs shell (AL9 reshape)

The editor core has two packaged participants:

- `editor.host` — `[transport]`, `[limits]`, `[overrides]`, plus
  `[default].client` (selects transport profile, so host).
- `editor.shell` — `[ui]`, `[editor]`, `[files]` (excluding
  `default.client`).

Files at `${root}/host.toml` and `${root}/shell.toml` for system,
user; `./.reovim/shell.toml` for project (no `host.toml` at project
layer).

A project-layer `host.toml` is rejected at file scan with WARN
(not silently ignored).

## 7. Force override

Three forms, all at editor-core boot time:

```
reovim --force-set <namespace>.<field>=<value> [...]
reovim --force-config <path/to/override.toml>
REOVIM_FORCE_CONFIG=<path/to/override.toml> reovim
```

Locked properties:

- Top precedence (layer 7).
- Bypasses overlay eligibility (may set host-class).
- Logged at WARN per overridden field at boot:
  `force-override: <ns>.<field> = <value> (was: <prior>, source: <layer>)`.
- Single-process. Never persisted, never inherited by children.
- Visible in introspection (`source = "force"`).
- `secret` fields redacted in the WARN line.
- Unknown field → ERROR at parse.

## 8. Schema vocabulary

Schema is TOML, one table per field. Reference example:

```toml
[__meta]
namespace      = "module.vim"
schema_version = "1.0"

[fields.leader]
type            = "string"
default         = "<space>"
trust           = "shell"
project_overlay = true
lifecycle       = "boot"
description     = "Leader key for vim-mode chord prefixes."

[fields.scroll-margin]
type            = "u32"
default         = 4
range           = "0..=64"
trust           = "shell"
project_overlay = true
lifecycle       = "boot"
description     = "Lines kept visible above and below the cursor."

[fields.api-token]
type            = "string"
default         = ""
trust           = "host"
project_overlay = false
secret          = true
lifecycle       = "boot"
description     = "Auth token for the upstream service."
```

Vocabulary table:

| Attribute | Required | Values |
|---|---|---|
| `type` | yes | `bool`, `i32`, `u32`, `i64`, `u64`, `f64`, `string`, `path`, `enum:<a>\|<b>\|...`, `list<T>`, `table<K,V>` |
| `default` | yes (unless `required = true`) | literal of declared type |
| `range` | numeric only | `A..=B`, `>=A`, `<=B` |
| `trust` | yes | `host`, `shell`, `module-private` |
| `project_overlay` | no (defaults from `trust`) | `true`, `false` |
| `lifecycle` | no (default `boot`) | `boot`, `reloadable`, `session` (v1 honors `boot` only) |
| `secret` | no (default `false`) | `true`, `false` |
| `required` | no (default `false`) | `true`, `false` |
| `description` | yes | prose |

List/table bounds (required for `list<*>` and `table<*,*>`):
`max-len`, `elem-max`, `max-entries`, `max-key-bytes`,
`max-value-bytes`. Without explicit bounds, editor-core caps from §11
apply.

`__meta` is reserved across the vocabulary.

## 9. Bootstrap order

```
A. Discover and validate editor.host. Defaults from packaged schema;
   layers 2..7 read in order. host config is required to:
   - resolve library root (§5 in 1.3),
   - resolve auth/transport profile,
   - resolve runtime caps for the rest of the stack.

B. Discover and validate editor.shell. Same.

C. Apply force-overrides into an editor-core override map; log WARN.

D. Resolve PM lockfile and discover modules/drivers from library root.

E. For each module, run §12 (loader behaviour). Failure: PM6 Failed.

F. For each driver, same. Failure: driver tombstoned.

G. Start the runtime.
```

`CFG9` locks the bootstrap order: editor.host materialises before
any other participant.

## 10. Env-var encoding

Schema field names are `[a-z0-9-]+` (no underscores). Namespace
components match the same regex. This makes the env-var encoding
reversible **by construction** — no escaping, no greedy-match
ambiguity, nothing for a future maintainer to mis-parse:

- prefix: `REOVIMCFG_`
- each namespace dot → `__` (double underscore)
- each hyphen → `_` (single underscore)
- letters uppercased

Example: `module.vim.leader-key` ↔
`REOVIMCFG_MODULE__VIM__LEADER_KEY`.

`__` cannot appear inside a single segment, so the split is
unambiguous.

For `list<T>` env override, indices append:
`REOVIMCFG_MODULE__VIM__BINDINGS__0__KEY`.
For `table<K, V>`, keys encode the same way and append.

Env-var override of a field whose schema name violates the regex
fails at schema-load (§4). The percent-encoding alternative was
rejected: it buys unconstrained schema names at the cost of a
grammar humans cannot type and shells mangle. The name-regex
constraint is the cheaper invariant to hold for decades.

`reovim config sources` prints the env-var form for any
env-sourced field (7.4 §7), so users never construct it by hand.

> **CFG10 — Env-var encoding is the reversible kebab grammar.**
> Schema names match `[a-z0-9-]+` per segment; the encoding is
> exactly the four steps above; decode(encode(x)) == x for every
> legal field path. No alternative or legacy encoding exists.
> *Class*: editor-core-enforced + CI round-trip fixture.


## 11. Bounded resources

| Resource | Cap | Error |
|---|---|---|
| Schema TOML size | 64 KiB | `SchemaTooLarge` |
| Single user/project/system file size | 256 KiB | `ConfigFileTooLarge` |
| Force-config file size | 256 KiB | `ForceConfigTooLarge` |
| Merged `ConfigSlice` total bytes | 256 KiB | `ConfigSliceTooLarge` |
| Per-field string value | 4 KiB | `StringFieldTooLarge` |
| Per-field path value | 4 KiB | `PathFieldTooLarge` |
| `list<*>` length default | 1024 | `ListTooLong` |
| `table<*,*>` entries default | 1024 | `TableTooLarge` |
| Nested table depth | 8 | `NestingTooDeep` |

Participants may declare narrower limits in schema. Wider participant
limits are clamped.

## 12. Loader behaviour

For each cdylib at load time:

1. Read symbols (4 required + optional `MIGRATE_FROM`).
2. Check `REOVIM_CONFIG_ABI_VERSION` against the editor-core pinned version.
   Mismatch → `IncompatibleConfigAbi`.
3. Parse schema; validate vocabulary, threat model (§5),
   name regex. Any failure → `SchemaInvalid`.
4. Add participant to registry under declared namespace.
   Collision → `NamespaceConflict`.
5. Run layer stack for participant's namespace.
6. Validate merged result against schema (type, range, enum,
   required).
7. Construct `ConfigSlice` (see `06-ABI/04-Config-Slice.md`).
   Size > §11 → `ConfigSliceTooLarge`.
8. Pass slice to participant init.

A participant that does not export config symbols is treated as
having an empty schema and receives no slice.

## 13. Validation lifecycle

| Point | What is validated | Fail-mode |
|---|---|---|
| Schema parse | syntax, vocabulary, threat model, name regex | `SchemaInvalid` |
| Layer merge — type | each layer matches declared type | participant rejected |
| Layer merge — range | numeric in range; list/table in bounds | participant rejected |
| Layer merge — enum | string is a declared variant | participant rejected |
| Layer merge — required | every required field has a value | participant rejected |
| Slice construction | total size; UTF-8 in strings | `ConfigSliceTooLarge` / `Utf8Invalid` |
| Runtime field access | nothing (slice is pre-validated) | n/a |

### 13.1 Unknown-field policy per layer

| Layer | Policy |
|---|---|
| 1 (defaults) | impossible — defaults come from schema |
| 2 (system) | WARN once per field, ignore |
| 3 (user) | WARN once per field, ignore |
| 4 (project) | ERROR — project unknown fields fail validation |
| 5 (env) | WARN once per var, ignore |
| 6 (CLI) | ERROR — `--set` of unknown field fails launch |
| 7 (force) | ERROR — `--force-set` of unknown field fails launch |

### 13.2 Removed fields after schema upgrade

- Migration shim runs and result is validated. No warning.
- No shim, minor mismatch: WARN, drop the value.
- No shim, major mismatch: ERROR.

## 14. Schema versioning

`REOVIM_CONFIG_SCHEMA_VERSION` is `(major << 16) | minor`.

| Change | Major bump | Minor bump | Migration |
|---|---|---|---|
| Add optional field with default | NO | YES | none |
| Add required field | YES | NO | participant-supplied |
| Remove field | YES | NO | values dropped (§13.2) |
| Rename field | YES | NO | participant-supplied |
| Tighten range | YES | NO | values may fail validation |
| Loosen range | NO | YES | none |
| Change `trust` | YES | NO | participant-supplied |
| Change `type` | YES | NO | participant-supplied |
| Change `secret` | NO | YES | none (editor core applies redaction) |
| Change `lifecycle` | NO | YES | none |

## 15. User-file metadata

```toml
# Single-namespace file:
[__meta]
schema_version = "1.0"

[]                          # implicit: file-namespace = file location
leader = "<space>"

# Multi-namespace file:
[module.vim.__meta]
schema_version = "1.2"

[module.vim]
leader = "<space>"

[driver.render-tui.__meta]
schema_version = "2.0"

[driver.render-tui]
profile = "256-color"
```

If `__meta` is absent, the editor core assumes user file targets the current
schema version; mismatch is detected at validation.

## 16. Project-root resolver

For v1, project-layer directory is the launcher's `cwd` at editor-core
startup. This is provisional.

> The project-root directory used to resolve layer 4 is determined
> by a `ProjectRootResolver`. v1 ships exactly one resolver,
> `Cwd`, which returns the launcher's cwd. Future versions may
> ship session-scoped resolvers; the layer-stack contract does not
> change.

## 17. Redaction

`secret = true` redacts in:
- `reovim config dump` (any flavour),
- DS12 logs,
- ErrorBuf diagnostics,
- crash reports,
- debug recordings,
- force-override WARN lines.

Bypass: `reovim config dump --unsafe-show-secrets`. Emits its own
WARN.

Storage in `ConfigSlice` is unredacted — participant sees real value.

## 18. Locked rules

> **CFG1 — Participants declare a schema before init.** Every
> config-bearing cdylib exports the symbols in §12 step 1. Loader
> rejects on missing/malformed schema, ABI mismatch, threat-model
> violation, or namespace conflict before participant init runs.
> *Class*: editor-core-enforced.

> **CFG2 — Schema defaults are authoritative.** The schema's
> per-field `default` is the single source of truth. There is no
> separate defaults blob symbol.
> *Class*: editor-core-enforced.

> **CFG3 — Seven-layer precedence is global.** Order: defaults,
> system, user, project, env, CLI, force. No participant may
> change it.
> *Class*: editor-core-enforced.

> **CFG4 — Project overlay is schema-gated.** Project-file
> participation applies only to fields with
> `project_overlay = true`. A project file containing a host-class
> section fails parse with `IllegalProjectHostSection`.
> *Class*: editor-core-enforced.

> **CFG5 — Force override sits at top, logs and redacts.** §7
> properties.
> *Class*: editor-core-enforced.

> **CFG6 — `ConfigSlice` is normalised KVP.** See
> `06-ABI/04-Config-Slice.md`.
> *Class*: ABI / runtime.

> **CFG7 — Secret fields redacted by default everywhere.** §17.
> *Class*: editor-core-enforced.

> **CFG8 — Threat-model fields MUST be host-class.** §5. A schema
> that violates the rule fails load with `IllegalTrustClass`.
> *Class*: editor-core-enforced.

> **CFG9 — Bootstrap host config materialises first.** §9. `EditorInit`
> resolves `editor.host` (and the host-only minimum needed for
> library-root and auth) before any module/driver discovery.
> *Class*: editor-core-enforced.

> **CFG10 — Env-var encoding is the reversible kebab grammar.**
> Body in §10. All config field and namespace names use
> kebab-case (`[a-z0-9-]+`) everywhere in the spec and
> implementation; no snake_case config keys exist.
> *Class*: editor-core-enforced + CI round-trip fixture.

## Open items

1. ~~Env-var encoding alternative~~ — resolved (§10):
   reversible kebab grammar locked as CFG10; percent-encoding
   rejected; all spec field names swept to kebab-case.
2. Reloadable lifecycle for v1.x — schema attribute reserved but
   v1 honors `boot` only. When does the editor core gain reload paths?
3. Whether `pkgs` namespace fully participates in the layer stack
   or remains gated solely by PM4. Current §2 says it participates;
   confirm against `07-Surfaces/01-Package-Manager.md` rules.
4. `[default].client` placement (host vs shell). Currently placed in host
   (§6); confirm against AL2 reshape note in 1.3 §6.

## Conformance

| Rule | Fixture |
|---|---|
| CFG1 | Cdylib without config symbols loads fine, gets no slice. Cdylib with malformed schema is rejected with `SchemaInvalid` and never runs init. |
| CFG2 | Schema with `default` field; absent in user file → slice carries default. |
| CFG3 | Layer fixtures cover every step; force wins over CLI; CLI wins over env; etc. |
| CFG4 | Project file with `[transport]` section (host class) → `IllegalProjectHostSection`. |
| CFG5 | `--force-set` of an `[overrides]` field → applied + WARN logged. |
| CFG6 | Round-trip: editor core constructs slice; participant accessor reads same values. |
| CFG7 | `secret = true` field shows `<redacted>` in `dump`; `--unsafe-show-secrets` reveals + WARN. |
| CFG8 | Schema with shell-class command-string field → load rejected. |
| CFG9 | Host config sentinel (e.g. `[runtime].library-root`) read before any module discovery in trace order. |
| CFG10 | Encode/decode round-trip over every field path in every registered schema; fixture with a hyphen-rich path (`module.vim.leader-key`); schema with `snake_case` name rejected at load. |
