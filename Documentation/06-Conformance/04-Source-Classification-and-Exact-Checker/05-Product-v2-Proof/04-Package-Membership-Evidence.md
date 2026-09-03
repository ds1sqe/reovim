# Package Membership Evidence

[Product v2 Proof](../05-Product-v2-Proof.md)

## Cargo source-package membership

The stable evidence source is one bounded before/after capture pair for each
shipping package. Foundation and Kernel Interface package-list output is
captured immediately before and after the complete four-stage compiler
journey. All four invocations use the same locked, offline, absolute manifest,
exact package, and Cargo identity and must preserve each package's semantic
path set. Exact-revision evidence forbids a dirty-package override.

The parser requires UTF-8 and final LF. It rejects empty, duplicate, absolute,
parent-escaping, backslash, NUL, CR, embedded-LF, overlong, over-count, and
over-byte paths. The report retains each raw-output digest and a
deterministically
sorted semantic set.

Each capture retains its own raw digest and byte count plus its package pair's
common canonical semantic-set digest. Raw line order may differ within a pair
when the sorted member set and semantic digest remain exact; requiring
identical raw bytes would contradict Cargo's set-valued package-list evidence.

The Product-closure registry maps every expected member as:

- `copied`, with one exact tracked Product/Production source path; or
- `cargo_generated`, with one closed Cargo-owned expected name.

Every copied member selects a regular non-symlink repository file matching
pre/post inventory, digest, size, owner, provenance, and use. Each shipping
package declares an explicit include list. It uses an SPDX license expression
and either no README or one explicit Product-owned package README. A root or
workspace README, external license file, Tool/Test/Example source, validation
consumer, helper, or undeclared asset entering either package fails. Bundling
promotes a non-production file to `Production`; a label cannot waive that
consequence.

The combined evidence proves Cargo-selected member paths and the exact tracked
repository bytes designated as copied sources. It does not prove that Cargo's
staged copy retained those bytes, normalized/generated member bytes, `.crate`
archive bytes, or a release bundle. Any such claim fails with
`PACKAGE_ARCHIVE_PROOF_UNAVAILABLE` until a bounded staging/archive mechanism
reads and hashes every member.
