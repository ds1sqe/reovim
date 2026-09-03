# Compiler Units, Inputs, and Correlation

[Product v2 Proof](../05-Product-v2-Proof.md)

## Stable compiler evidence

Exact locked/offline metadata preflight resolves declared packages, targets,
features, and dependency kinds before project/helper execution. Metadata is not
actual compiler-unit evidence.

The current profile executes four exact library selections in one shared Cargo
home and target authority: Foundation shipping, Foundation validation, Kernel
Interface shipping, then Kernel Interface validation. Each names the absolute
manifest, package, `--lib`, target, `--profile release`, features, `--jobs 1`,
JSON message format, and authorized target directory. This is a closed ordered
matrix, not a registry-driven build graph.

The cleared environment fixes canonical Cargo/rustc/checker, offline mode,
incremental-off, prepared Cargo/temp/trace paths, and rejection of conflicting
Cargo configuration. It fixes `LD_LIBRARY_PATH` to the one inventoried
`/compiler/lib` directory because the relocated rustc image cannot rely on its
host-relative `$ORIGIN` path inside the prepared root; no other dynamic-library
search directory is admitted.

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

Only the Foundation shipping stage records the exact
Version/Host/Target/Version discovery sequence. The later three stages reuse
the same authorities and record no repeated probe. Each stage compiles one
selected unit. Its Cargo artifact-event set is exactly that unit plus the fresh
upstream events implied by the registered direct and transitive relations.
Freshness does not establish a dependency edge.

The three direct compiler edges are Foundation validation to Foundation,
Kernel Interface shipping to Foundation, and Kernel Interface validation to
Kernel Interface. The last stage also carries a transitive fresh Foundation
event but no direct Foundation `--extern`. Each direct `--extern` selects its
exact retained provider `rmeta`.

Every compiler unit retains separate `rlib` and `rmeta` identities. Cargo may
report a top-level shipping `rlib` and later fresh hashed provider `rlib`;
their paths remain distinct and their digest/size equality is proved for both
shipping providers. The report retains every correlated Cargo artifact event
with unit, direct-or-transitive role, freshness, and exact filenames. It also
retains both top-level/hashed/provider-metadata joins.

Standalone verification repeats both distinct-path/equal-byte provider joins,
all direct `rmeta` joins, the exact fresh-event matrix, complete ordered rustc
grammar, and dep-info/sysroot rewrite. Recognizing a target, emit flag, or
rustc basename is insufficient.

Every supplied `--extern` is conservatively participating even when rustc may
not use it. Every provider maps to another proved allowed Product unit or the
bound sysroot. Every repository source/module path reported by dep-info maps to
one exact classified pre/post-identical input. Their four-stage union equals
the four registered compiler-source sets. Visibility of
`/repo/conformance/product-closures.json` changes neither that union nor its
members. Its presence in dep-info, package membership, a supplied `--extern`,
or a Product closure fails. Any other unknown, excluded, out-of-root,
Repository Tooling, generated, vendored, or `OUT_DIR` input also fails.
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
