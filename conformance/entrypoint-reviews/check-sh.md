# Entrypoint Review: `scripts/check.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:719dd1d266ddfcf5a5c91f8f4ffa90067189fc7e0e46351d3d7647f8ef5372af`
- Review mechanism: exact content and retained review

## Direct local invocations

- `scripts/check-style.sh` by direct path execution
- `scripts/check-documentation.sh` by direct path execution
- `scripts/check-conformance.sh` by direct path execution

Cargo commands are external-tool invocations. Their selected workspace targets
are governed by the workspace topology; the checker binary's direct invocation
is reviewed at the `scripts/check-conformance.sh` boundary.

## Reviewed selection

The default contributor path retains workspace library/binary tests, every
checker integration except the two named exhaustive matrices, the depgraph
integration target, one real candidate report, doctests, Clippy, and rustdoc.
The exact-Git and full Product-empty CLI matrices remain direct pre-checkpoint
commands. Their omission from the per-edit path is exact and closed; another
skip or target change reopens this review.

The selected warm target is 75 seconds on Rust 1.92 and stable. The hosted job
has a five-minute outer timeout that also covers dependency setup and exact-
revision evidence.

## Exhaustive checkpoint commands

Run the retained matrices directly before a qualifying checkpoint:

```sh
cargo test --locked --offline -p reovim-conformance \
    --test checker --all-features \
    product_empty_v2_full_cli_and_mutation_matrix -- --exact
cargo test --locked --offline -p reovim-conformance \
    --test checker --all-features \
    exact_git_generate_verify_and_raw_identity_mutants -- --exact
```

## Assumptions

- `/usr/bin/env` resolves a Bash implementation supporting the constructs used
  by these exact bytes.
- `BASH_SOURCE` resolves this checked-out script so `ROOT` selects the same
  repository whose local entrypoints are admitted.
- Cargo, Rustdoc, Rustfmt, and Clippy operate on the exact locked workspace and
  toolchain selected by the invoking environment.
- The Clippy invocation binds `CLIPPY_CONF_DIR` to the reviewed repository root
  so ambient configuration cannot disable the three-level nesting threshold.

## Decision

Admit these exact bytes and the three declared direct local invocation edges for
Product-empty v2 Repository Tooling checks. Any byte or edge change reopens
this review.
