# CI Action Review

Date: 2026-09-02. This record covers the two third-party GitHub Actions used
by the hosted contributor workflow. They are CI tooling, not Cargo dependencies,
Product admission, release qualification, or evidence publication.

Workflow digest: `sha256:033269df23b9ff12496f77f1730b663a17cd3932044a094af1352ce7119e10e5`.
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
  boundaries. The contributor lane uses the selected Cargo and Rustc directly;
  a separate exact journey records their identities when its environment is
  admitted.

The hosted workflow runs only the bounded contributor gate. It does not claim
exact evidence: the mutable hosted image has no committed execution-tool
binding or retained namespace/prepared-root measurement. Exact evidence
remains a separate fail-closed clean-revision journey in an admitted
environment.

## Update gate

Any action SHA, tag, input, permission, runner image, toolchain matrix, or
network behavior change reopens this review. Re-run workflow syntax validation
and both toolchain contributor lanes. Separately run clean exact generation and
verification in an admitted environment. An update does not authorize evidence
upload, publication, or a Product claim.
