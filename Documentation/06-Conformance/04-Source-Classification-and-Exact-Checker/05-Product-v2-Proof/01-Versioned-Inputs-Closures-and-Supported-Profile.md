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
5. `reovim-product-closures/v1`; and
6. `reovim-execution-tool-reviews/v1`.

The generated `reovim-source-role-map/v2` binds every input/schema/tool/lock/
compiler-substrate digest and records active Product closure evidence. Exact
checker-capability qualification separately publishes
`reovim-product-proof-gate/v1`. A fixture-gate artifact cannot serve as the
Source Role Map for real Product source.

Schema dispatch is exact. A v2 verifier never upgrades, fills, or reinterprets
a v1 policy, topology, dependency review, or Source Role Map.

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

`reovim-product-closures/v1` names each shipping or nonshipping validation
closure, its workspace/manifest/lock, target/root/kind/crate types, target
triple/profile/features, package-membership policy, public-consumer relation,
and closed mechanism profile. An empty registry is valid only when inventory
and topology contain no Product owner or shipping root. A Product path without
an exact closure and evidence fails; an unmatched closure also fails.

## Initial shipping and validation profiles

The first shipping profile is exactly one tracked, Authored,
`Product + Production` library producing `rlib` under Cargo `release` for
`x86_64-unknown-linux-gnu`, with explicit empty/default-empty features and only
the bound Rust sysroot. Its source package uses explicit copied members and
closed Cargo-generated names.

The paired validation profile is a separate Product-owned `Test` library
consumer producing `rlib` through one allowed inward Product-role edge. It is
absent from shipping compilation and package membership. Validation ownership
grants no shipping authority.

Preflight rejects external packages; normal/build/dev/proc-macro dependencies
in the shipping closure; custom builds, `links`, build scripts, proc macros,
binary/example/benchmark targets, custom targets, native/link/search flags or
assets, response files, generated/vendored Product source, `OUT_DIR`,
`include!`, `include_str!`, `include_bytes!`, `env!`, and `option_env!`.
Rejection occurs before project/helper code can execute.

Ordinary external `mod` files are supported when dep-info reports them and all
Product checks cover them. The syntax profile enumerates an exact compiler-
attribute allowlist. Any admitted built-in derive is a separate exact list;
the consumer must exercise each generated public trait. Every other attribute,
derive, or macro is unavailable.

The following chapters own the prepared execution root, compiler correlation,
package membership, public consumer, and claim limits for this closed profile.
