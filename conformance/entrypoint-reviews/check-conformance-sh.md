# Entrypoint Review: `scripts/check-conformance.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:11c728c8ccf691dbeaa461b604556da98974a59be27f48a8ad91246db6eaa0b7`
- Review mechanism: exact content and retained review

## Direct local invocations

- Cargo target `reovim-conformance/reovim-conformance` by `cargo run --bin`

The script also builds the same target and resolves Cargo, Rustc, and Git as
external tools. It passes the Product-closure and execution-tool admissions to
the checker and selects the committed Git execution environment for exact
mode. Configuration and report paths passed to the binary are consumed
inputs, not additional executable entrypoints.

## Assumptions

- `/usr/bin/env` resolves a Bash implementation supporting the constructs used
  by these exact bytes.
- `BASH_SOURCE` resolves this checked-out script and the supplied Cargo home
  contains only the dependency state intended by the calling lane.
- Cargo selects the exact declared package and binary target from the locked
  workspace when given `-p reovim-conformance --bin reovim-conformance`.

## Decision

Admit these exact bytes and the declared Cargo-target edge for candidate and
exact Product-empty v2 conformance. Any byte, target, environment binding, or
edge change reopens this review.
