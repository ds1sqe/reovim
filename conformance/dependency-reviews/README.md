# Repository Tooling Dependency Reviews

This directory records the human rationale behind the exact machine-readable
admissions in [`../dependency-reviews.json`](../dependency-reviews.json). The
registry and `Cargo.lock` are enforcement inputs; these records explain why a
specific locked package is admitted. An update to any package, feature,
checksum, license, resolved consumer/profile, or closure requires a new review.

These dependencies belong only to the non-shipping `RepositoryTooling`
partition. Product packages retain no third-party dependency admission through
these records, and first-party code remains `#![forbid(unsafe_code)]`.

## Records

- [`serde-json-closure.md`](serde-json-closure.md) — metadata-resolved
  `serde_json 1.0.151`, `serde_core 1.0.229`, `itoa 1.0.18`, `memchr 2.8.3`,
  and `zmij 1.0.23`; lock-only `serde 1.0.229` and `serde_derive 1.0.229`.
- [`syn-closure.md`](syn-closure.md) — `syn 3.0.4`, `proc-macro2 1.0.107`,
  `quote 1.0.47`, and `unicode-ident 1.0.24`, all metadata-resolved.
- [`nix-linux-execution-closure.md`](nix-linux-execution-closure.md) — selected
  Linux-only process and prepared-root closure for the repository checker;
  admitted only for the current supervisor and fixed Product two-pair proof,
  and invalidated by any consumer, API, feature, or closure change.

The RustSec observation date is 2026-09-01 at advisory database revision
`72f8b23d78ea6c4c9ded301a4c6ec4260e8b4c27`. “No match” means only that the
reviewed lock had no indexed advisory at that database revision; it is not an
unsafe-code, correctness, provenance, or supply-chain audit.

The exploratory timings referenced by both records were not retained with raw
artifacts and were not correctness-equivalent. They are explicitly
nonconforming observations, not performance evidence or a budget.

## Bounded-input snapshot and headroom

Measured on the integrated 2026-09-01 candidate tree, excluding the exact
`.git`, `target`, and `tmp` roots: 168 files / 2,313,287 bytes total and 21
Rust files / 606,598 bytes. These counts identify the review snapshot; they are
not a performance result.

The committed limits deliberately leave bounded refoundation headroom: 16,384
observed entries, 8,192 files and per-directory entries, 64 MiB aggregate
source, and 4 MiB per source file. Configuration is capped at 1 MiB with JSON
depth 128 and 50,000 aggregate container items. Cargo metadata/report output is
16 MiB, child stderr 1 MiB, tool executables 64 MiB, paths 4,096 bytes, post-
parse `syntax_item_expr_type_visits` callbacks 1,000,000, findings 4,096, and
each owned child has a 60-second absolute supervision deadline with a 55-second
execution/termination trigger.
These are safety ceilings with roughly one to two orders of magnitude over the
current file/byte baseline where practical; they are neither latency/memory
targets nor permission to grow up to the cap. Any increase changes report
identity and requires a new measured review.
