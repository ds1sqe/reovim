# serde_json Closure Admission

Status: admitted only for the non-shipping Reovim conformance checker.

## Exact need and feature selection

The checker consumes Cargo metadata and three committed JSON contracts. It
needs bounded JSON syntax parsing into `serde_json::Value`, followed by
first-party closed-field conversion. The direct dependency is exactly:

```toml
serde_json = { version = "=1.0.151", default-features = false, features = ["std"] }
```

No derive, arbitrary-precision, raw-value, preserve-order, or unbounded-depth
feature is enabled. The checker separately rejects duplicate keys, enforces a
committed nesting/item bound, and retains serde_json's recursion limit.

## Exact locked closure

Present in Cargo metadata resolve on all three committed
`x86_64-unknown-linux-gnu` profiles:

- `serde_json 1.0.151` — MIT OR Apache-2.0;
- `serde_core 1.0.229` — MIT OR Apache-2.0;
- `itoa 1.0.18` — MIT OR Apache-2.0;
- `memchr 2.8.3` — Unlicense OR MIT;
- `zmij 1.0.23` — MIT.

Lock-only/metadata-unresolved: `serde 1.0.229` and `serde_derive 1.0.229`, both
MIT OR Apache-2.0. The machine registry binds each package's exact crates.io
source, checksum, resolved consumers, and resolved-profile set; `Cargo.lock`
binds metadata-unresolved records as well.

## Safety, build, and runtime review

The metadata-resolved closure is pure Rust in the native-link sense: no package
declares `links`, and no packaged C, C++, or assembly source was found. Build
scripts are declared by `serde_json`, `serde_core`, and `zmij`. Unsafe-related
Rust source was observed in every resolved package; `memchr` and `zmij` contain
architecture-specific intrinsics. This is a source observation, not proof that
a compiler unit selects every occurrence. The unsafe/build surface is admitted
only behind RepositoryTooling process and source bounds, never as a product
dependency.

Declared MSRVs are no higher than Rust 1.71, below Reovim's Rust 1.92 floor.
The exact versions were non-yanked and their upstream repositories active at
review time. Publication ownership is concentrated, so any update reopens
maintenance and provenance review.

`cargo-audit 0.22.2` found no matched advisory or warning on 2026-09-01 using
RustSec database revision
`72f8b23d78ea6c4c9ded301a4c6ec4260e8b4c27`.

## Cost and decision

An exploratory fresh release check of JSON + Syn observed 2.68 s wall, 5.30 s
user, 0.88 s system, 279,000 KiB maximum RSS, and 9,593,545 target bytes. The
probe used a discarded, unbounded, non-equivalent program and retained no raw
artifacts. These numbers are nonconforming observations and select no budget.

Decision: admit this exact closure for the checker. Reject `cargo_metadata` for
this responsibility; Cargo's versioned CLI JSON is authoritative and the
extra wrapper/closure is unnecessary.

Primary references: [Cargo metadata](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html),
[`serde_json 1.0.151`](https://docs.rs/serde_json/1.0.151/serde_json/), and
[RustSec review revision](https://github.com/RustSec/advisory-db/commit/72f8b23d78ea6c4c9ded301a4c6ec4260e8b4c27).
