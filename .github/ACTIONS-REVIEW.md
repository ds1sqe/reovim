# CI Action Review

Date: 2026-09-02. This record covers the two third-party GitHub Actions used
by the hosted tooling workflow. They are CI tooling, not Cargo dependencies,
Product admission, release qualification, or evidence publication.

Workflow digest: `sha256:b92cb695e19e195514f6f2485d101a2b77b46cc148ede5d86e3a952441f9961c`.
The admission guard accepts only this reviewed canonical YAML form; any workflow
byte change reopens action extraction and review before the digest is updated.

## `actions/checkout`

- Upstream: <https://github.com/actions/checkout>
- Pin: `3d3c42e5aac5ba805825da76410c181273ba90b1` (`v7.0.1`)
- Need: materialize the exact GitHub event revision before repository checks.
- Authority: job permissions are `contents: read`; checkout receives GitHub's
  ephemeral read-only `github.token` to fetch the event revision, while
  `persist-credentials: false` prevents retaining it afterward. The workflow
  requests neither submodules nor Git LFS and never commits, pushes, publishes,
  or uploads evidence.
- Trust/runtime: maintained by GitHub under the MIT license. The pinned action
  uses GitHub-hosted action runtime facilities on the mutable `ubuntu-24.04`
  image boundary.

## `dtolnay/rust-toolchain`

- Upstream: <https://github.com/dtolnay/rust-toolchain>
- Pin: `6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772` (`v1`, reviewed current master)
- Need: install the declared Rust `1.92.0` and current-stable matrix toolchains
  with Rustfmt and Clippy.
- Authority: the composite action may invoke `rustup` and download the selected
  toolchain/components. It receives no explicitly passed secret, has no
  repository write permission, and does not publish artifacts.
- Trust/runtime: maintained by David Tolnay under the MIT license. Rustup and
  the selected toolchain distribution are explicit network/supply-chain
  boundaries; the checker records the actual Cargo and Rustc identities used.

Exact evidence uses a fresh owner-only `0700` directory per matrix job. The
checker consumes that root once under its held authority marker; the workflow
does not reuse or upload it.

## Update gate

Any action SHA, tag, input, permission, runner image, toolchain matrix, or
network behavior change reopens this review. Re-run workflow syntax validation,
both toolchain lanes, candidate conformance, and clean exact generate/verify.
An update does not authorize evidence upload, publication, or a Product claim.
