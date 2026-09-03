# Entrypoint Review: `scripts/check.sh`

## Scope

- Kind: script
- Exact content digest:
  `sha256:10cba8a2ef7cee4aa2cf2848bfd99a28a895e5de11f5bd855863d834d633692d`
- Review mechanism: exact content and retained review

## Direct local invocations

- `scripts/check-style.sh` by direct path execution
- `scripts/check-documentation.sh` by direct path execution
- `scripts/check-cache.sh` by direct path execution

Cargo commands are external-tool invocations. Their selected workspace targets
are governed by the workspace topology.

## Reviewed selection

`--full` retains workspace library/binary tests, every checker integration
except the two named exhaustive matrices, the depgraph integration target,
doctests, Clippy, and rustdoc. It also formats, checks, executes the public-
boundary integration journey, lints, and documents all Product packages
through their separate locked workspace, explicit release target, and root-
excluded build directory.

The default path derives package roots and reverse local dependencies from the
two locked Cargo workspaces. It compares current Git-blob file identities with
an ignored successful local record bound to direct tool bytes, command,
target, supported Cargo configuration, and relevant environment identities.
Missing or malformed state, unknown ownership, graph uncertainty, or a global
input runs `--full` semantics.

A known file change selects its owning package and transitive reverse
dependents. Only direct packages run formatting. Every selected package runs
its complete check, test, lint, doctest where applicable, and rustdoc surface.
Selection never propagates upstream: a TUI-only change selects only the TUI
suite, while a Kernel change also selects every dependent Client suite.
Documentation and file-local style have their own selected paths.

When selector preparation succeeds, the script snapshots inputs and metadata
before execution and revalidates them afterward, including on a hit. It
publishes a sealed local record only after every selected/full check passes and
both post-check witnesses match. Preparation uncertainty still runs full but
is unpublishable. Gate failure, endpoint drift, or an ABA metadata change
leaves prior state intact. Git commits each durable blob identity. The ignored
local record is an optimization, not evidence.

The exact-Git and explicit Product-empty fixture matrices remain direct pre-
checkpoint commands. Their omission from the per-edit path is exact and
closed; another skip or target change reopens this review.

The full warm target is 75 seconds on Rust 1.92 and stable. The hosted job has
a five-minute outer timeout that also covers dependency setup. CI and final
checkpoints pass `--full`; active Product evidence remains a separate exact
clean-revision journey.

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
- Injected Git repository namespaces and Ripgrep configuration are removed;
  system/global Git configuration is disabled for deterministic local tools.
- Cargo, Rustdoc, Rustfmt, and Clippy operate on the respective explicit locked
  Repository Tooling or Product workspace and selected toolchain.
- Git supplies tracked/untracked inventory and blob identities. Core Perl
  `JSON::PP` parses only Cargo's bounded local metadata used for selection.
- Local reuse does not bind opaque dynamic-loader, system-library, or compiler-
  sysroot state; `--full` remains the checkpoint and CI boundary.
- Product commands use the explicit Product manifest, target triple, release
  profile, and a toolchain-specific build directory below root `target/`.
- Exhaustive matrices use the release checker because the debug image is not
  admitted by the 96 MiB executable-identity bound.
- The Clippy invocation binds `CLIPPY_CONF_DIR` to the reviewed repository root
  so ambient configuration cannot disable the three-level nesting threshold.

## Decision

Admit these exact bytes and the three declared direct local invocation edges for
bounded full and component-selected contributor checks. Candidate evidence is
intentionally absent. Any byte or edge change reopens this review.
