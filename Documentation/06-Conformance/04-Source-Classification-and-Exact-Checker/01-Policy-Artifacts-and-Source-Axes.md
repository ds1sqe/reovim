# Policy Artifacts and Source Axes

[Source Classification and Exact Checker](../04-Source-Classification-and-Exact-Checker.md)

## Classification policy and generated evidence

Every evidence set carries an exact-revision Source Role Map generated from
separate schema-valid classification policy, exact workspace topology,
entrypoint reviews, and dependency admissions. Phase-B v1 reports only
all-`RepositoryTooling` dependency reachability and fails closed on Product or
shipping input. The [exact-execution chapter](02-Exact-Execution-Topology-and-Dependency-Evidence.md)
supplies the detailed execution, topology, and review definitions; the
[Phase-B gate](04-Phase-B-Scope-and-Product-Gate.md) supplies the detailed
scope and deliberately unproved Product boundary.

The Layer Model owns product-role and dependency semantics. This Conformance
contract owns two distinct versioned source-classification artifacts:

1. committed `conformance/source-classification.json`, validated against
   `conformance/source-classification.schema.json`, is policy input with schema
   identity `reovim-source-classification/v1`; and
2. every implementation or release evidence set publishes generated
   `reovim-source-role-map/v1` evidence, validated against committed
   `conformance/source-role-map.schema.json`, under the stable bundle key
   `conformance/source-role-map.json`.

Committed policy records scope roots and explicit exclusions, ordered path
rules and precedence, Source Owner, Source Provenance, Source Use, unmatched
policy, allowed product-role edges, selected shipping roots, and the generated
evidence key. It contains no source revision, checker result, graph result, or
conformance claim. The generated Source Role Map records the exact source
revision, policy digest, checker/Cargo/Rustc/lock/graph/source identities,
target and feature profile, enumerated classifications and exclusions,
concrete and owner edges, dependency reachability, findings, result, and report
identity. It never serves as its own policy input. Its identity inputs also bind
exact topology, the entrypoint-review and dependency-review registries, all
four input-schema digests, and its report-schema digest. Arrays are
deterministically sorted, and report identity is SHA-256 over canonical JSON
with the `report_identity` field omitted.

Standalone report verification validates the retained report's schema,
canonical identity, mode, and result consistency. It does not re-run semantic
validation against unavailable policy, topology, review, source, or tool
preimages. Those preimages remain retrievable from the report's exact revision
or the owning evidence-retention system when reproduction is required.

## Source Owner, Provenance, and Use

Source Owner is the closed tagged union `Product(ArchitecturalRole)` or
`RepositoryTooling`; the latter is non-product and absent from the product DAG.
Source Provenance is `Authored`, `Generated`, or `Vendored`. Generated and
vendored inputs carry upstream identity, digest, and a closed digest subject.
For Generated provenance, the checker recomputes the declared upstream-input
digest from its exact normalized path set; it does not prove the mechanism-
specific relation to generated output. For Vendored provenance, it recomputes
all content selected by the winning rule. Source Use has closed meaning:

- `Production` enters a selected, compiled, invoked, re-exported, or bundled
  product shipping closure;
- `Test` and `Example` belong only to declared non-shipping targets;
- `Tool` is repository mechanism or its declarative
  policy/configuration/evidence input outside every product closure; and
- `BuildOutput` is generated non-source output, never a label for generated
  source consumed by another target.

Product owners permit `Production`, `Test`, and `Example`. `RepositoryTooling`
permits `Tool`, `Test`, `Example`, and `BuildOutput`, never `Production`. Source
classification does not replace Documentation's Contract Authorities or grant
a dependency edge.

Source Use is single-valued: product shipping participation is `Production`;
otherwise repository-mechanism participation is `Tool`; `Test`/`Example` apply
only to test-/example-only paths. Inline test items do not relabel a Tool path,
while a separate declared test-target source is `Test`.

Phase-B v1 admits Generated provenance only for root `Cargo.lock`, whose special
mechanism check is locked Cargo metadata. Every other Generated path emits
`GENERATED_MECHANISM_PROOF_UNAVAILABLE` until its own relation proof exists.
