# Entrypoint Review: `scripts/check-conformance.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:4642722343c9446b87332659470128f3d1e774be70545c6dadaa2fe2663c7074`
- Review mechanism: exact content and retained review

## Direct local invocations

- Cargo target `reovim-conformance/reovim-conformance` by `cargo run --bin`

The script also builds the same target and resolves Cargo, Rustc, and Git as
external tools. Configuration and report paths passed to the binary are
consumed inputs, not additional executable entrypoints.

## Assumptions

- `/usr/bin/env` resolves a Bash implementation supporting the constructs used
  by these exact bytes.
- `BASH_SOURCE` resolves this checked-out script and the supplied Cargo home
  contains only the dependency state intended by the calling lane.
- Cargo selects the exact declared package and binary target from the locked
  workspace when given `-p reovim-conformance --bin reovim-conformance`.

## Decision

Admit these exact bytes and the declared Cargo-target edge for candidate and
exact Phase-B conformance. Any byte, target, or edge change reopens this review.
