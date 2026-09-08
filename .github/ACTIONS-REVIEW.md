# CI Action Review

This record covers the third-party GitHub Actions used by the contributor
workflow. They are CI tooling, not product dependencies or release evidence.
Action pins and permissions are reviewed with the workflow diff. There is no
second workflow hash or canonical-YAML byte assertion.

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

## Shared local and GitHub runner

`ci.json` owns the exact `rust-stable` and dated `rust-nightly` testing pins.
`./scripts/ci.sh` validates that manifest, runs both roles locally, and reports
failure after attempting both lanes. GitHub discovers the same roles/pins
through `--matrix`, then invokes `--toolchain NAME` for each lane with
`fail-fast: false`. The runner owns the exact toolchain/component/target setup,
locked Product fetch, and uncached `./scripts/check.sh --full` invocation.
There is no separate local copy of the CI command sequence or lane list.

Running a lane installs its declared toolchain, Rustfmt, Clippy, and
hosted/portable targets with Rustup's minimal profile when needed. It does not
self-update Rustup or change ci.json. The runner prints the resolved compiler
version; setup failure stops that lane rather than using another toolchain.
--help and --matrix perform no installation or network access. The manifest
has a closed schema: exact stable version and dated nightly, without hashes
or generated timestamps. This testing window does not change Product's
declared minimum Rust version or qualify that minimum separately.

Window updates are explicit manifest changes, followed by both complete lanes
before committing. Record the upstream version/date review with that change.
The local matrix checks that every lane uses its starting window and rejects
a changed manifest window at completion.
The pins initially represent the selected latest stable and nightly releases;
they do not silently advance as upstream publishes new versions.

GitHub's [Ubuntu 24.04 image inventory][runner-image] includes the Rustup
bootstrap. Local contributors must install Rustup before running the script.
The former `dtolnay/rust-toolchain` action is removed: toolchain setup now
belongs to the same runner used locally. Rustup and its official distribution
are explicit network/supply-chain boundaries. No artifact is published.

Each lane owns a fresh temporary Cargo home, fetches locked dependencies, and
runs checks offline. The runner removes only that temporary directory on exit,
never the caller's Cargo home. Build artifacts remain in check.sh's existing
compiler-specific target directories. Cargo/Rust compiler/formatter overrides,
Git configuration overrides, and child shell/Perl/ripgrep injection variables
are
cleared, as are Rustup distribution-server overrides. Cargo configuration
above the repository is rejected; repository
configuration remains part of the checked inputs. Rustup's installed toolchain
home is retained. No shared dependency cache or credentials are imported.

Parity means the same declared toolchains, prerequisites, fetch, and checks
on x86_64 Linux. It does not promise identical OS images, system libraries,
network, or inherited non-build environment. This runner is not a sandbox for
hostile startup files.
GitHub uses `ubuntu-24.04`; local bootstrap setup is owned by the contributor.
Failures use stable `CI_*` diagnostics and nonzero statuses.

The portable library check is compile evidence, not proof that an editor runs
on an unnamed custom host. Hosted tests do not qualify a release. The previous
exact-revision checker and workflow-byte admission machinery are retired.

## Update gate

Any action SHA, tag, input, permission, runner image, toolchain matrix, or
network behavior change reopens this review. Re-run workflow syntax validation
and both toolchain contributor lanes. An update does not authorize artifact
upload, publication, or a release claim. Keep repository permissions read-only
and checkout credentials nonpersistent unless separately approved.

[runner-image]: https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md
