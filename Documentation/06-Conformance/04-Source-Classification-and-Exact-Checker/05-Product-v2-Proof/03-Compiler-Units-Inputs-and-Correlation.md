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
of conflicting Cargo configuration. It fixes `LD_LIBRARY_PATH` to the one
inventoried `/compiler/lib` directory because the relocated rustc image cannot
rely on its host-relative `$ORIGIN` path inside the prepared root; no other
dynamic-library search directory is admitted.

Metadata and package discovery select the exact checker image as a private
rustc launcher. That launcher admits only the closed Rust 1.92 discovery
probe grammar, clears the rustc child environment, and supplies the one
compiler-library directory. It cannot accept a compiler unit. Actual builds
continue through the separate traced `RUSTC_WRAPPER`, which normalizes Cargo's
loader-path mutation back to the same exact directory before invoking rustc.

The exact checker binary acts as private `RUSTC_WRAPPER`. It bounds and records
original/effective argv, rejects response files and native, framework, link,
or unexpected search flags, and admits only the two exact prepared Cargo
dependency search paths. It rejects output/incremental/temp paths outside the
authority, requires one `--emit` specification, and assigns a unique private
dep-info path without removing release outputs. Original Cargo argv contains
no sysroot selection; effective argv adds exactly `--sysroot /sysroot`, whose
target libdir is the separately inventoried prepared-root substrate.

The coordinator requires a complete Cargo JSON stream, one successful build
terminal, `fresh: false`, no build-script event, and only expected artifacts.
It correlates wrapper units by workspace/package/target root, crate name/kind,
target, features/cfgs, outputs, and artifact paths.

For the initial Cargo profile, the fresh shipping build records the exact
Version/Host/Target/Version discovery sequence before its unit. Validation
reuses the same Cargo home, build directory, and target authority, so it
records no repeated discovery probes and exactly one consumer unit. Both unit
`rlib` and `rmeta` outputs retain separate identities. Cargo may report a
top-level shipping `rlib` and later a fresh hashed provider `rlib`; their paths
remain distinct and their digest/size equality is proved. The compile-only
consumer's `--extern` selects the exact retained provider `rmeta`, not the
shipping `rlib`.

The report retains every correlated Cargo artifact event with its unit,
freshness, and exact two-file `rlib`/`rmeta` path set. It separately retains
the top-level shipping `rlib`, hashed provider `rlib`, and provider `rmeta`
identities. Standalone verification repeats the distinct-path/equal-byte join,
the validation fresh-provider join, and the consumer-`rmeta` join. It also
revalidates the complete ordered rustc grammar and exact dep-info/sysroot
rewrite; recognizing a target, emit flag, or rustc basename is insufficient.

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
