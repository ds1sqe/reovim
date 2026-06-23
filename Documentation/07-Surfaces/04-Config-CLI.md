# 7.4 — Config CLI

**Scope.** The user-facing surface of the config service:
`reovim config dump | help | validate | set | force-set`.

**Heritage.** the q3-config-system RFC v2 §21 (folded).

**Locked rules.** None new; this chapter exposes CFG1..CFG9.

---

## 1. Command set

```
reovim config dump      [--effective] [--namespace <ns>] [--unsafe-show-secrets]
reovim config help      <namespace> [<field>]
reovim config validate  [--file <path>] [--namespace <ns>]
reovim config set       <namespace>.<field>=<value> [...]
reovim config force-set <namespace>.<field>=<value> [...]
reovim config sources   [--namespace <ns>]
```

`set` is a launcher convenience that writes to the user's
`shell.toml` (or appropriate file). Distinct from `--set` which
applies CLI layer 6 for one launch.

## 2. `dump`

Default: prints schema-default values for every participant.
`--effective`: prints the merged result for the current process
(or for the caller's specified launch context). Per-field source
tag and redaction (per CFG7) included:

```toml
[module.vim]
leader            = "<space>"            # source = user
replay-max-wait-ms = 0                   # source = force        (was: 150)
disable-recording = false                # source = default
api_token         = "<redacted>"         # source = user, secret
```

`--unsafe-show-secrets` reveals secret values; emits a WARN line
to stderr.

## 3. `help`

Renders schema descriptions, ranges, defaults, trust class,
lifecycle, secret flag.

```
$ reovim config help module.vim leader
Field:        module.vim.leader
Type:         string
Default:      "<space>"
Trust:        shell
Project overlay: yes
Lifecycle:    boot
Description:  Leader key for vim-mode chord prefixes.
```

## 4. `validate`

Runs the layer stack on a candidate file (or current files) without
booting the runtime. CI gate for user dotfiles.

```
$ reovim config validate --file ./.reovim/shell.toml
OK   module.vim.leader = "<space>"
ERR  module.vim.replay-max-wait-ms = 99999  (out of range 50..=60000)
```

Exits non-zero if any field fails.

## 5. `set` and `force-set`

`set` writes to the matching file:

```
$ reovim config set module.vim.leader="\\"
# writes to $XDG_CONFIG_HOME/reovim/modules/vim.toml
```

`force-set` is a one-shot invocation that runs the launcher with
the override applied (CFG5):

```
$ reovim config force-set editor.host.overrides.disable-runtime-unload=true
WARN: force-override: editor.host.overrides.disable-runtime-unload = true (was: false, source: default)
```

## 6. `sources`

Dumps which file (or layer) each field's value came from:

```
$ reovim config sources --namespace module.vim
leader            : user             ($XDG_CONFIG_HOME/reovim/modules/vim.toml)
replay-max-wait-ms: project          (./.reovim/modules/vim.toml)
disable-recording : default
```

Useful for "where is this coming from?" debugging.

## 7. Env-var encoding

Per 1.5 §10, env-var encoding is reversible:

- prefix `REOVIMCFG_`
- dots → `__`, hyphens → `_`, uppercase
- `module.vim.leader-key` ↔ `REOVIMCFG_MODULE__VIM__LEADER_KEY`

The CLI accepts and emits both forms. `reovim config sources`
prints the env-var name when a field's source is `env`:

```
api_token : env (REOVIMCFG_MODULE__VIM__API_TOKEN)
```

## 8. JSON output

For tooling integration, every `config` subcommand accepts
`--json`:

```
$ reovim config dump --effective --json --namespace module.vim
{
  "namespace": "module.vim",
  "fields": [
    { "name": "leader", "type": "string", "value": "<space>",
      "source": "user", "secret": false },
    ...
  ]
}
```

## Open items

1. Whether `set` accepts a multi-line TOML payload from stdin (for
   bulk edits). Default: no — one field per invocation.
2. Whether `force-set` chains arguments to a wrapped command:
   `reovim config force-set <kv> -- reovim --tui` (apply override
   then exec subcommand). Default: no — `--force-set` on the
   target launcher is the canonical path.
3. Path completion / shell completion files. Convention TBD.
4. Whether `validate` warns on unknown fields per 1.5 §13.1 layer
   policy. Default: yes — mirrors server policy.

## Conformance

| Behaviour | Fixture |
|---|---|
| `dump --effective` | Set fields across multiple layers; verify per-field source tags. |
| Secret redaction | `secret = true` field hidden by default; `--unsafe-show-secrets` reveals. |
| Env round-trip | Encode `module.vim.leader-key` to env var; decode → equal. |
| `validate` | Fixture user file with one out-of-range field; `validate` exits non-zero with ERR row. |
| `sources` | Set field via env, project, force; verify sources reports each. |
