# Phase-B Scope and Product Gate

[Source Classification and Exact Checker](../04-Source-Classification-and-Exact-Checker.md)

## Durable shipping closure

The durable shipping-closure contract includes selected product targets;
normal, build, and proc-macro reachability; source inclusion and macro/include
expansion; compiled generated source; public re-exports; invoked build/package
helpers; and bundled files/assets. Dev/test placement does not waive product
ownership, and a non-production target entering a package becomes `Production`.
No closure node may be Repository Tooling.

## Phase-B fail-closed boundary

Phase-B v1 proves only the current all-`RepositoryTooling`, empty-shipping-root
topology and reports workspace dependency reachability, not shipping closure.
Any Product owner or nonempty shipping root emits
`PRODUCT_BOUNDARY_PROOF_UNAVAILABLE`. Product target/source/use, generated-code
expansion, packaging, public-surface, helper, and asset obligations—including
product applicability of `ARCH-001`—remain unproved. No Product crate may enter
until a later checker revision supplies compiler and package evidence.

The durable Product-capable checker obligation treats Cargo/build metadata as
dependency-edge authority and checks it against policy and generated evidence;
prose grants no exception. Classification and projection are path- and target-
aware. Cross-partition edges fail before the ArchitecturalRole-only product
projection; concrete package and product-owner graphs remain acyclic. Crate-
level edges are not sufficient proof. Transitive closure, source/import,
include/macro, invocation/packaging, and public-type checks reject forbidden
mechanisms even when a package has an allowed name. Unsupported syntax or
generated semantics fail closed unless a bounded compiler-produced evidence
source proves them. Phase-B v1 supplies none of the Product proof this
paragraph requires.

Temporary exceptions live in explicit tracker/evidence records and never become
an unreported allowlist or second edge registry.
