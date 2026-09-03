# Exact Execution, Topology, and Dependency Evidence

[Source Classification and Exact Checker](../04-Source-Classification-and-Exact-Checker.md)

## Phase-B v1 execution environment and Git identity

Exact v1 mode uses a cleared environment and records sanitized absence of Cargo
home configuration. It checks only whether Cargo-home `config` or `config.toml`
exists, without opening, hashing, or reporting either path. It never
intentionally opens, hashes, lists, or reports credential paths. Cargo receives
canonical Cargo home under offline mode; v1 does not claim instrumentation
proof of Cargo's internal reads. Exact CI uses a fresh credential-free home.

Cargo subprocesses use `/` as their recorded working directory. The checker
records absence of `/.cargo/config` and `/.cargo/config.toml`; the absolute
committed manifest supplies repository identity.

Host metadata subprocesses fix `CARGO_TARGET_DIR` to
`target/cargo-metadata` under the canonical repository root. That directory is
Repository Tooling scratch below the already excluded root `target/`; it is
not Product input, compiler evidence, or report evidence. This prevents Cargo
target-discovery caches from materializing below a Product workspace and
changing the next source inventory.

Exact mode binds the tracked Git tree's names, blob identities, and modes as
`git_tree_digest` and requires equality with classified inventory and directly
hashed worktree bytes/modes. It requires the raw stage-0 index map to equal
`HEAD`, rejects non-`H` index flags, and rejects hierarchical attributes or
repository info-attributes before a Git command could apply content filters.

## Phase-B v1 topology and bounded execution

`conformance/workspace-topology.json`, validated as
`reovim-workspace-topology/v1`, enumerates the singleton workspace manifest/
members, every manifest/lock, Cargo configuration, build scripts, custom target
specifications, linker assets, repository entrypoints, required profiles, and
every package/target with exact Cargo-reported `src_path`. Addition, omission,
or metadata mismatch fails. It is neither classification policy nor generated
evidence.

`reovim-entrypoint-reviews/v1` maps every script/workflow topology entrypoint
one-to-one to exact content identity, mechanism, explicit direct repository-
local invocations, assumptions, disposition, and retained review. Missing,
extra, stale, unclassified, digest-drifted, or unresolved entries fail. A Cargo
target is a terminal reference proved by topology/Cargo/source evidence, not a
falsely digest-free source review.

For the all-tooling topology, v1 begins at every Cargo-reported target root and
conservatively resolves ordinary external `mod` declarations against complete
classified inventory. Every inventoried Rust source is a target root or
reachable through that supported closure. Conditional modules are potentially
selected; missing/ambiguous modules and `#[path]` fail. This is supported-
syntax source-selection evidence, not compiler-active-unit evidence.

The topology owns positive hard limits for configuration/report/tool bytes;
JSON depth/items; child captures and execution/supervision deadlines; path and
aggregate/per-directory entries; source files/per-file/aggregate bytes;
selected syntax visits; and findings. Syntax visits charge selected Syn
callbacks and do not bound parser peak memory. A CLI may tighten but never
widen limits. Every declared profile executes exactly once in exact mode; a
subset is diagnostic only.

The child deadline triggers termination. One greater absolute supervision
deadline covers spawn, bounded capture, kill, direct-child reap, and reader
joins. Ownership cleanup uncertainty aborts before report emission.

## Phase-B v1 dependency admission

`reovim-dependency-reviews/v1` binds the exact singleton lock digest and maps
every resolved or lock-only external Cargo identity/checksum one-to-one to
license, metadata-resolved incoming consumers/profiles, disposition, and
retained review. A metadata-unresolved admission has empty resolved sets.
Missing, extra, consumer/profile-drifted, or stale entries fail; a lock entry is
not implicit admission. Metadata-resolved means conservative presence or an
incoming Cargo resolve edge, not compiler-active participation.

Product v2 multi-workspace topology, full descendant ownership, prepared-root
limits, and execution-tool admission are defined only by the
[Product v2 Proof](../04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof.md).
