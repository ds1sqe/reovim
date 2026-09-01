# Policy Artifacts and Source Axes

[Source Classification and Exact Checker](../04-Source-Classification-and-Exact-Checker.md)

## Classification policy and generated evidence

Every evidence set carries an exact-revision Source Role Map generated from
version-matched, schema-valid classification policy, exact workspace topology,
entrypoint reviews, and dependency admissions. Phase-B v1 reports only
dependency reachability for an all-`RepositoryTooling` topology and fails
closed on Product or shipping input.

The Layer Model owns product-role and dependency semantics. This chapter owns
the general classification/evidence separation and Phase-B v1 artifacts. The
[Product v2 Proof](../04-Source-Classification-and-Exact-Checker/05-Product-v2-Proof.md)
owns additional Product-closure, execution-tool, compiler, package, and public-
consumer evidence without changing a v1 report's meaning.

Phase-B v1 has two distinct source-classification artifacts:

1. committed `conformance/source-classification.json`, validated against
   `conformance/source-classification.schema.json`, is policy input with schema
   identity `reovim-source-classification/v1`; and
2. generated `reovim-source-role-map/v1` evidence, validated against committed
   `conformance/source-role-map.schema.json`, uses stable bundle key
   `conformance/source-role-map.json`.

Committed v1 policy records scope roots/exclusions, ordered path rules and
precedence, Source Owner, Source Provenance, Source Use, unmatched policy,
allowed product-role edges, selected shipping roots, and generated evidence
key. It contains no source revision, checker result, graph result, or claim.

The generated v1 Source Role Map records exact revision, policy/topology/
review/schema/checker/Cargo/Rustc/Git/lock/source identities, profiles,
classifications/exclusions, packages/targets, concrete and owner edges,
metadata dependency reachability, processes, findings, result, and report
identity. It never serves as policy input. Arrays are deterministically sorted;
report identity is SHA-256 over canonical JSON with its identity field omitted.

Standalone v1 verification validates retained schema, canonical identity,
mode, and result consistency. It does not re-run semantics against unavailable
policy, topology, review, source, or tool preimages. Schema dispatch is exact:
a later verifier never fills or reinterprets a v1 report as Product evidence.

## Source Owner, Provenance, and Use

Source Owner is the closed tagged union `Product(ArchitecturalRole)` or
`RepositoryTooling`; the latter is non-product and absent from the product DAG.
Source Provenance is `Authored`, `Generated`, or `Vendored`. Generated and
vendored inputs carry upstream identity, digest, and a closed digest subject.
For Generated provenance, an upstream digest proves those input bytes, not the
mechanism-specific relation to output. For Vendored provenance, the checker
recomputes all content selected by the winning rule.

Source Use has closed meaning:

- `Production` enters a selected, compiled, invoked, re-exported, or bundled
  Product shipping closure;
- `Test` and `Example` belong only to declared nonshipping targets;
- `Tool` is repository mechanism or its declarative
  policy/configuration/evidence input outside every Product closure; and
- `BuildOutput` is generated non-source output, never generated source consumed
  by another target.

Product owners permit `Production`, `Test`, and `Example`. `RepositoryTooling`
permits `Tool`, `Test`, `Example`, and `BuildOutput`, never `Production`. A
registered nonshipping validation library remains Product-owned `Test` and
gains no shipping authority. Source classification does not replace Contract
Authority or grant a dependency edge.

Source Use is single-valued. Inline test items do not relabel a Tool or
Production path, while a separate declared test/validation target is `Test`.
A Test or Example path entering a shipping package becomes `Production` or is
rejected.

Phase-B v1 admits Generated provenance only for root `Cargo.lock`, whose closed
mechanism check is locked Cargo metadata. Every other Generated path emits
`GENERATED_MECHANISM_PROOF_UNAVAILABLE`. Product generated-source requirements
are stricter and live in the Product v2 claim chapter.
