# Versioned Inputs, Closures, and Supported Profile

[Product v2 Proof](../05-Product-v2-Proof.md)

## Product-proof authority

Every Product shipping or validation closure is selected by version-matched,
schema-valid Product-closure and execution-tool admissions and is proved by
correlated compiler-reported Product source/dependencies, Cargo package
membership where shipping, and external-crate public-consumer evidence.
Unsupported mechanisms fail closed, and fixture capability never qualifies
active Product.

This Product-specific obligation is inseparable from the general Source Role
Map and dependency laws. It uses these committed inputs:

1. `reovim-source-classification/v2` policy;
2. `reovim-workspace-topology/v2` exact multi-workspace topology;
3. exact `reovim-entrypoint-reviews/v1`;
4. `reovim-dependency-reviews/v2`, partitioned by workspace/lock;
5. `reovim-product-closures/v2`; and
6. `reovim-execution-tool-reviews/v1`.

For the active profile, `conformance/product-closures.json` is also an explicit
file in the closed preflight repository inventory. It remains classified as
`RepositoryTooling` with `Tool` use. Its admission as proof configuration does
not make it Product source, a package member, or a Product closure member.
The raw file identity is the authority transported into prepared execution;
the parsed registry remains the semantic equality target described below.

The generated `reovim-source-role-map/v2` binds every input/schema/tool/lock/
compiler-substrate digest and records active Product closure evidence. Exact
checker-capability qualification separately publishes
`reovim-product-proof-gate/v1`. A fixture-gate artifact cannot serve as the
Source Role Map for real Product source.

Schema dispatch is exact. A v2 verifier never upgrades, fills, or reinterprets
a v1 policy, topology, Product registry, dependency review, or Source Role
Map. The frozen `authored-std-lib/v1` Foundation proof does not qualify the
active `authored-std-lib/v2` profile.

## Multi-workspace and closure selection

Topology v2 enumerates one or more workspaces. Each owns a unique Product or
Repository Tooling partition, manifest, lock, members/packages/targets,
configuration/build/link assets, and exact target/profile/feature selections.
Every package and target belongs to one workspace with its exact Cargo-reported
root. The root workspace is not implicitly Product.

Product and Repository Tooling use separate workspaces and locks. Every Cargo
evidence command selects its manifest, package, target, target triple, profile,
and features explicitly. Working-directory/default-member/default-feature
behavior and undeclared workspace inheritance grant no evidence.

`reovim-product-closures/v2` names each shipping or nonshipping validation
closure, its workspace/manifest/lock, target/root/kind/crate types, target
triple/profile/features, package-membership policy, public-consumer relation,
and closed mechanism profile. Each public relation contains a required,
sorted `reexports` set. Every entry names the local public type, its direct
provider relation, and provider type. Foundation has no entries. Kernel
Interface has the complete registered Foundation origins used by its authored
source.

The registry is an equality target, not source authority. Static and exact
proof derive actual aliases independently and require complete equality.
Missing, additional, retargeted, or locally replaced aliases fail even when a
replacement has the same public path, kind, and built-in derives.

An empty registry is valid only when inventory and topology contain no Product
owner or shipping root. A Product path without an exact closure and evidence
fails; an unmatched closure also fails.

## Supported shipping and validation profile

The active `authored-std-lib/v2` profile is exactly the Foundation and Kernel
Interface shipping libraries and their two nonshipping validation libraries.
Each produces `rlib` under Cargo `release` for
`x86_64-unknown-linux-gnu`, with explicit empty/default-empty features and only
its admitted Product edges plus the bound Rust sysroot. Both shipping packages
use explicit copied members and closed Cargo-generated names.

Foundation has no Product dependency. Kernel Interface has the one direct
normal Foundation edge. Each separate Product-owned `Test` consumer has one
direct normal edge to its shipping library. Validation libraries are absent
from shipping compilation and package membership. Validation ownership grants
no shipping authority.

Preflight rejects external packages and every shipping dependency except the
exact unconditional normal Kernel Interface-to-Foundation edge. Foundation
dependencies and all shipping build/dev/proc-macro edges fail. Custom builds,
`links`, build scripts, proc macros, binary/example/benchmark targets, custom
targets, native/link/search flags or assets, response files, generated or
vendored Product source, `OUT_DIR`, `include!`, `include_str!`,
`include_bytes!`, `env!`, and `option_env!` also fail. Rejection occurs before
project or helper code can execute.

Ordinary external `mod` files are supported when dep-info reports them and all
Product checks cover them. The syntax profile enumerates an exact compiler-
attribute allowlist. Any admitted built-in derive is a separate exact list;
the consumer must exercise each generated public trait. Every other attribute,
derive, or macro is unavailable.

The following chapters own the prepared execution root, compiler correlation,
package membership, public consumer, and claim limits for this closed profile.
