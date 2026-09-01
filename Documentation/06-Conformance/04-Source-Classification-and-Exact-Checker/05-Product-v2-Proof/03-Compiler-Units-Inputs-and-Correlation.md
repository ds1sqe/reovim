# Compiler Units, Inputs, and Correlation

[Product v2 Proof](../05-Product-v2-Proof.md)

## Stable compiler evidence

Exact locked/offline metadata preflight resolves declared packages, targets,
features, and dependency kinds before project/helper execution. Metadata is not
actual compiler-unit evidence.

One fresh shipping build names exact absolute manifest, package, `--lib`,
target, `--profile release`, features, `--jobs 1`, JSON message format, and
authorized target directory. The validation library has a separate fresh
selection. The cleared environment fixes canonical Cargo/rustc/checker,
offline mode, incremental-off, prepared Cargo/temp/trace paths, and rejection
of conflicting Cargo configuration.

The exact checker binary acts as private `RUSTC_WRAPPER`. It bounds and records
original/effective argv, rejects response files and every native/link/search
flag, rejects output/incremental/temp paths outside the authority, requires one
`--emit` specification, and assigns a unique private dep-info path without
removing release outputs.

The coordinator requires a complete Cargo JSON stream, one successful build
terminal, `fresh: false`, no build-script event, and only expected artifacts.
It correlates wrapper units by workspace/package/target root, crate name/kind,
target, features/cfgs, outputs, and artifact paths.

Every supplied `--extern` is conservatively participating even when rustc may
not use it. Every provider maps to another proved allowed Product unit or the
bound sysroot. Every repository source/module path reported by dep-info maps to
one exact classified pre/post-identical input. Unknown, excluded, out-of-root,
Repository Tooling, registry, generated, vendored, or `OUT_DIR` input fails.
Environment dependencies and include inputs are unavailable in the first
profile and fail if reported.

Compiler identity binds canonical Cargo/rustc/checker plus a bounded exact
inventory/digest of rustc compiler-support files and the selected target
libdir. Executable version text or digest alone is not the compiler substrate.
The exposure manifest proves admitted visibility, not every compiler/runtime
file actually read; host runtime prerequisites remain named environment
evidence rather than falsely compiler-owned bytes.

The resulting evidence proves compiler-reported Product source/module inputs,
actual Cargo artifact units, and conservative supplied dependency edges for the
closed profile. It does not claim a complete compiler filesystem-read trace,
expanded source, or item-level cfg selection.
