# Phase-B Scope and Product Gate

[Source Classification and Exact Checker](../04-Source-Classification-and-Exact-Checker.md)

## Durable shipping closure

The durable shipping-closure contract includes selected product targets;
normal, build, and proc-macro reachability; source inclusion and macro/include
expansion; compiled generated source; public re-exports; invoked build/package
helpers; and bundled files/assets. Dev/test placement does not waive product
ownership, and a non-production target entering a package becomes `Production`.
No closure node may be Repository Tooling.

## Historic Phase-B fail-closed boundary

Phase-B v1 proves only its exact all-`RepositoryTooling`, empty-shipping-root
topology and workspace dependency reachability, not shipping closure. Any
Product owner or nonempty shipping root emits
`PRODUCT_BOUNDARY_PROOF_UNAVAILABLE`. Product target/source/use, generated-code
expansion, packaging, public-surface, helper, asset, and product applicability
of `ARCH-001` remain unproved.

The durable Product checker treats Cargo/build metadata as dependency-edge
authority and checks it against policy and generated evidence; prose grants no
exception. Classification and projection are path- and target-aware. Cross-
partition edges fail before product-role projection; concrete package and owner
graphs remain acyclic. Crate-level edges alone do not prove source/import,
include/macro, invocation/packaging, or public-type closure.

Phase-B v1 supplies none of that Product proof. It retains only its original
exact-revision meaning and cannot be dispatched, filled, or reinterpreted as a
v2 report. Its generic unavailable finding never admits, partially admits, or
summarizes Product source.

The [Product v2 Proof](../04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof.md)
owns the versioned mechanism-specific admission boundary. Temporary exceptions
remain explicit tracker/evidence records and never become an unreported
allowlist or second dependency registry.
