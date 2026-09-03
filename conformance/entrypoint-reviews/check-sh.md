# Entrypoint Review: `scripts/check.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:bf7cfe4cbfc25b22d2333d50bd6f654fe29de1b96d6fe45c7fbd6f9cbc0f5c04`
- Review mechanism: exact content and retained review

## Direct local invocations

- `scripts/check-style.sh` by direct path execution
- `scripts/check-documentation.sh` by direct path execution

Cargo commands are external-tool invocations. Their selected workspace targets
are governed by the workspace topology.

## Reviewed selection

The default contributor path retains workspace library/binary tests, every
checker integration except the two named exhaustive matrices, the depgraph
integration target, doctests, Clippy, and rustdoc. It also formats, checks,
executes the public-boundary integration journey, lints, and documents all
four Product packages through their separate locked workspace, explicit
release target, and root-excluded build directory.

The exact-Git and explicit Product-empty fixture matrices remain direct pre-
checkpoint commands. Their omission from the per-edit path is exact and
closed; another skip or target change reopens this review.

The selected warm target is 75 seconds on Rust 1.92 and stable. The hosted job
has a five-minute outer timeout that also covers dependency setup. Active
Product evidence remains an exact clean-revision checkpoint journey.

Product build and documentation outputs are separated among Rust 1.92,
explicit stable, and the otherwise active toolchain. Rustdoc 1.92 otherwise
can read a newer search index from the shared unversioned documentation
directory and panic. The fixed slot selection is not an arbitrary
user-selected target path.

## Exhaustive checkpoint commands

Run the retained matrices directly before a qualifying checkpoint:

```sh
cargo test --locked --offline -p reovim-conformance \
    --release --test checker --all-features \
    product_empty_v2_full_cli_and_mutation_matrix -- --exact
cargo test --locked --offline -p reovim-conformance \
    --release --test checker --all-features \
    exact_git_generate_verify_and_raw_identity_mutants -- --exact
```

## Assumptions

- `/usr/bin/env` resolves a Bash implementation supporting the constructs used
  by these exact bytes.
- `BASH_SOURCE` resolves this checked-out script so `ROOT` selects the same
  repository whose local entrypoints are admitted.
- Cargo, Rustdoc, Rustfmt, and Clippy operate on the respective explicit locked
  Repository Tooling or Product workspace and selected toolchain.
- Product commands use the explicit Product manifest, target triple, release
  profile, and a toolchain-specific build directory below root `target/`.
- Exhaustive matrices use the release checker because the debug image is not
  admitted by the 96 MiB executable-identity bound.
- The Clippy invocation binds `CLIPPY_CONF_DIR` to the reviewed repository root
  so ambient configuration cannot disable the three-level nesting threshold.

## Decision

Admit these exact bytes and the two declared direct local invocation edges for
bounded Repository Tooling and Product contributor checks. Candidate evidence
is intentionally absent. Any byte or edge change reopens this review.
