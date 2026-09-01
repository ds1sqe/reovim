# Exact Execution, Topology, and Dependency Evidence

[Source Classification and Exact Checker](../04-Source-Classification-and-Exact-Checker.md)

## Exact execution environment and Git identity

Exact mode uses a cleared environment and records sanitized absence of Cargo
home configuration. It checks only whether Cargo-home `config` or `config.toml`
exists, without opening or hashing either path and without reporting the
Cargo-home path. The checker never intentionally opens, hashes, lists, or
reports credential paths and does not treat their presence as pass/fail. Cargo
still receives canonical Cargo home under offline mode; v1 does not claim
instrumentation proof of Cargo's internal reads. Exact CI uses a fresh,
credential-free Cargo home.

Cargo subprocesses use `/` as their recorded working directory and the checker
records absence of `/.cargo/config` and `/.cargo/config.toml`; the absolute
committed manifest path supplies repository identity.

Exact mode binds the full tracked Git tree's names, blob identities, and modes
as `git_tree_digest` and requires equality with the classified inventory and
directly hashed worktree bytes/modes. It requires the raw stage-0 index
mode/object-ID/path map to equal `HEAD`, rejects non-`H` index flags, and
rejects hierarchical attributes or repository info-attributes before any Git
command that could apply content filters.

## Workspace topology and bounded execution

Exact Cargo structure is a third, separate committed input:
`conformance/workspace-topology.json`, validated against
`conformance/workspace-topology.schema.json`, with schema identity
`reovim-workspace-topology/v1`. It enumerates the workspace manifest and
members, every manifest and lockfile, Cargo configuration, build scripts,
custom target specifications, linker assets, repository invocation entrypoints,
required target/feature profiles, and every package and Cargo target with its
exact Cargo-reported `src_path`. Addition, omission, or metadata mismatch in
that topology fails closed. It is neither classification policy nor generated
evidence.

`conformance/entrypoint-reviews.json`, validated against
`conformance/entrypoint-reviews.schema.json`, is the separate exact entrypoint-
admission registry. It maps every script/workflow topology entrypoint one-to-
one to its exact content identity, execution mechanism, explicit direct
repository-local invocation set, assumptions, `admit` disposition, and exact
retained review record. Missing, extra, stale, unclassified, or digest-drifted
entries and review records fail closed. Every declared local invocation must
resolve to a topology entrypoint. A Cargo target is a terminal reference
validated by topology, Cargo, and source evidence; this registry makes no
separate reviewed-source-closure claim for it. Changing entrypoint bytes or the
reviewed invocation set requires renewed review and synchronized registry/
topology identities.

For the current all-tooling Rust topology, v1 begins at every Cargo-reported
target `src_path` and conservatively resolves ordinary external `mod`
declarations against the complete classified inventory. Every inventoried Rust
source must be a target root or reachable through that supported module-source
closure. Conditional modules are treated as potentially selected; missing or
ambiguous module files and `#[path]` indirection fail closed. This is bounded
source-selection evidence for the supported syntax, not compiler-active-unit
evidence.

The topology input also owns positive hard limits for configuration, report,
and tool-executable bytes; JSON depth and container items; child stdout, stderr,
and execution/supervision deadlines; path and aggregate/per-directory entry
counts; source-file counts and bytes; aggregate source bytes; post-parse
`syntax_item_expr_type_visits`; and findings. That limit charges selected
Item/Expr/Type visitor callbacks, not every Syn node, and does not bound parser
peak memory. A CLI may only tighten these limits. Each profile deterministically
derives Cargo target and feature arguments from committed fields. Exact mode
executes and aggregates every declared profile exactly once; selecting fewer
profiles is diagnostic and cannot support a conformance claim.

The child deadline triggers termination; one greater absolute supervision
deadline covers spawn through bounded capture, kill, direct-child reap, and
reader joins. If ownership cleanup cannot be established within the latter,
the checker aborts before emitting a report.

## Dependency admission evidence

`conformance/dependency-reviews.json`, validated against
`conformance/dependency-reviews.schema.json`, is the exact third-party Cargo-
package admission registry. It binds the exact lockfile digest and maps every
resolved or lock-only external package identity and checksum one-to-one to its
license, Cargo-metadata-resolved incoming consumers and profiles, `admit`
disposition, and retained review record. A metadata-unresolved lock admission
has empty resolved sets. Missing, extra, consumer/profile-drifted, or otherwise
stale entries fail; a lockfile entry is not an implicit admission.

Metadata-resolved means conservative presence or an incoming edge in Cargo's
profile resolve; it does not claim a compiler-active unit.
