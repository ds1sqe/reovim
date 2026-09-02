# Entrypoint Review: `scripts/check-conformance.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:4a12f94aaad6159163fdd322515b6f1a1c2edd2d137b7647da3f79de07ae890c`
- Review mechanism: exact content and retained review

## Direct local invocations

- Cargo target `reovim-conformance/reovim-conformance` by the exact
  `held-cargo-target-exec` mechanism

The script opens the resolved Cargo executable on descriptor 8 and validates
the held inode. It executes `/proc/$$/fd/8 8<&-` for one locked offline build,
so Cargo and its descendants cannot inherit descriptor 8, then closes the
parent descriptor when the build returns. It opens and validates the built
checker on descriptor 9.

Before that build, the script rejects ambient Cargo/Rust compiler, wrapper,
target, rustflag, profile, and incremental overrides plus Cargo-home or root
configuration files. It runs Cargo from `/` with the absolute repository
manifest, explicit `x86_64-unknown-linux-gnu` target, and explicit target
directory. The held checker path is therefore derived from that exact target,
not from an ambient build-target selection or a possibly stale host path.

Both held images pass a `--version` execution probe through the same parent-fd
and child-close form before authoritative work. Procfs absence, LSM denial, or
an ineligible held image produces a stable `HELD_CARGO_*` or `HELD_CHECKER_*`
failure before a claim is generated.

Candidate generation and verification execute `/proc/$$/fd/9 9<&-`. Bash
expands `$$` to the parent shell that retains descriptor 9 while the child
closes its inherited descriptor before `execve`. Repeated calls therefore use
the same held image without passing that authority into the checker or its
descendants. Positive and near-miss lexical mutants plus a live Linux test bind
the exact form, pathname replacement behavior, repeated use, and descriptor
closure.

Rustc and Git remain external tools. Product-closure, execution-tool, report,
and evidence paths are consumed inputs rather than executable entrypoints.

## Assumptions

- `/usr/bin/env` resolves a Bash implementation supporting the constructs used
  by these exact bytes.
- `BASH_SOURCE` resolves this checked-out script and the supplied Cargo home
  contains only the dependency state intended by the calling lane.
- Linux procfs exposes held descriptors through `/proc/self/fd` and
  `/proc/<pid>/fd`, and Bash provides the expansion and child-redirection order
  exercised by the live retained test. A host denying this access is a typed
  unavailable outcome, not a fallback to pathname execution.
- Held Cargo honors the explicit absolute manifest, package, binary target,
  compilation target, target directory, Cargo home, and Rust compiler after
  the rejected environment/configuration set is proved absent.

## Decision

Admit these exact bytes and the declared held Cargo-target edge for candidate
and exact Product-empty v2 conformance. Any byte, descriptor form, target,
environment binding, or edge change reopens this review.
