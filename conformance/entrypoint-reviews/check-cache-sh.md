# Entrypoint Review: `scripts/check-cache.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:6654edd59c5bd3427365a8e0f38f09f4d277f84d45127f6b6db1c6c50b5b06ea`
- Review mechanism: exact content and retained review

## Direct local invocations

None. The helper is an internal child of `scripts/check.sh`. It executes only
hosted Git, Cargo, Rust toolchain, Perl, and core command-line tools.

## Reviewed behavior

- `prepare` binds reviewed command bytes, direct tool bytes and versions,
  supported build environment, local Git/Cargo configuration, and the fixed
  target/profile into one local context. System/global Git configuration and
  injected Git namespaces are removed before inventory operations.
- Every producer is byte-, record-, or file-bounded before materialization.
  Repository inputs are capped by file, aggregate, path, file-count, and
  directory-count limits. Oversize or unresolved context fails open to full.
- Locked, offline Cargo metadata supplies the two current package roots and
  local reverse-dependency edges. Package names are workspace-qualified and
  duplicate, unresolved, malformed, or out-of-repository graph data fails to
  full mode. Both metadata subprocesses bind and validate their reported
  target directory as invocation-owned scratch; ambient target settings cannot
  write discovery state below a Product workspace.
- Git supplies tracked and non-ignored untracked paths plus blob identities.
  The ignored state retains sorted path, owner, mode, size, and identity rows
  followed by a completeness seal. It is a local performance record, not
  conformance or qualification evidence.
- Known changes select their owners and transitive reverse dependents. The
  closure never propagates upstream. A generated graph proves TUI-only selects
  TUI, while Kernel selects Kernel and every dependent Client.
- Workspace manifests and locks select their workspace. Documentation owns its
  cross-file gate. Gate/config paths select full mode. Unknown ownership,
  missing state, malformed state, or graph/context uncertainty selects full.
- When preparation succeeds, `publish` recomputes tool context and repository
  state even for a hit. It compares content and file/directory metadata ABA
  witnesses before atomically replacing only a non-hit context record below
  `target/basic-check-cache`. Failure or drift preserves prior state.
- If context, graph, or initial inventory cannot be formed, `prepare` selects
  an unpublishable full run. Context-specific fixtures may be skipped there,
  while graph, transition, state, and publication fixtures still execute.
- `fixtures` checks metadata scratch isolation, hit/miss, exact TUI and Kernel
  plans, add/delete/rename, malformed/truncated state, context invalidation,
  failed replacement, and content/metadata drift. It derives fixture
  identities from fixture bytes.

## Assumptions

- `/usr/bin/env` resolves Bash with arrays, associative arrays, namerefs,
  process substitution, and `mapfile`.
- `BASH_SOURCE` resolves this script below the reviewed repository root.
- Git supports the reviewed inventory, identity, configuration, and repository
  discovery forms. Cargo supports locked offline metadata. Core Perl provides
  `JSON::PP` and the bounded stream parsers.
- The directly recorded hosted tools provide the exact forms used by these
  bytes. The selected Rust toolchain and default native linker/archive tools
  remain readable regular files during a reusable run.
- The caller supplies a fresh owner-only directory in which the selector
  creates its metadata target. Cargo reports that exact directory for both
  workspace graph queries, and caller cleanup removes it with the other
  selection scratch.
- The local context does not fingerprint dynamic loaders, system libraries,
  compiler sysroots, or other opaque runtime support. CI and final checkpoint
  verification therefore use uncached full mode.
- Repository root `target/` is ignored and is not Product or evidence state.

## Decision

Admit these exact bytes as the bounded internal selector and atomic local-
coverage writer for `scripts/check.sh`. It creates no Product, conformance, or
qualification claim. Any byte, ownership rule, graph rule, cache format, or
direct invocation edge change reopens this review.
