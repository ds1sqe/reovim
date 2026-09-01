# Entrypoint Review: `scripts/check.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:234396a2f80d16da0a5912ec6d07ab7601f8a01d53409444ca7d630e07711258`
- Review mechanism: exact content and retained review

## Direct local invocations

- `scripts/check-documentation.sh` by direct path execution
- `scripts/check-conformance.sh` by direct path execution

Cargo commands are external-tool invocations. Their selected workspace targets
are governed by the workspace topology; the checker binary's direct invocation
is reviewed at the `scripts/check-conformance.sh` boundary.

## Assumptions

- `/usr/bin/env` resolves a Bash implementation supporting the constructs used
  by these exact bytes.
- `BASH_SOURCE` resolves this checked-out script so `ROOT` selects the same
  repository whose local entrypoints are admitted.
- Cargo, Rustdoc, Rustfmt, and Clippy operate on the exact locked workspace and
  toolchain selected by the invoking environment.

## Decision

Admit these exact bytes and the two declared direct local invocation edges for
Product-empty v2 Repository Tooling checks. Any byte or edge change reopens
this review.
