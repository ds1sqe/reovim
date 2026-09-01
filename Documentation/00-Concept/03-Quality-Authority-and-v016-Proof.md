# Quality, Authority, and v0.16 Proof

> **Normative concept chapter.** Part of the
> [Editing-System Concept](../00-Concept.md).

## Mission-critical quality before breadth

Mission-critical quality is an evidence discipline, not a feature-count goal
or unsupported certification claim. Priority is:

1. source fidelity and prevention of silent corruption or loss;
2. atomic state authority and deterministic conflict handling;
3. explicit Effect Certainty, recovery, cancellation, and shutdown;
4. bounded memory, queue, concurrency, retry, and pressure behavior;
5. actionable, loss-visible diagnostics and auditable evidence;
6. measured reaction latency, memory, I/O, and extension overhead; and
7. feature breadth.

A feature enters a release candidate only with an owning public contract, no
privileged bypass, bounded work, applicable lifecycle and recovery semantics,
public-boundary and fault evidence, structured diagnostics, workload-specific
measurements, and a mapped qualification plan. Final inclusion additionally
requires the exact packaged candidate to pass that plan on every claimed
environment. Unsupported behavior fails closed with a typed outcome.

## Authority, evidence, and release scope

Each normative requirement has one Contract Authority and mapped verification
classes. Target-contract text is not implementation evidence. Implementation
conformance requires matching committed code plus applicable public-path tests,
fault cases, and measurements. Release qualification additionally requires the
exact packaged candidate on every environment for which qualification is
claimed.

The Supported and Qualified Environment Matrix records support and
qualification claims, prerequisites, exact artifacts, mapped conformance and
workloads, limitations, evidence status, and deterministic invalidators. The
first environment Reovim attempts to qualify is
`x86_64-unknown-linux-gnu`; this target selection is not a qualification claim.

## v0.16 vertical proof

The v0.16 vertical proof uses one partially resident, byte-addressed Resource
presented simultaneously through a faithful byte/hex Representation and a Text
Representation. Both directions edit one Buffer and one History; invalid byte
sequences remain intact and directly editable.

The proof covers:

- bounded open and access without default whole-Resource materialization;
- low-level and high-level edits, including preservation or rejection where a
  high-level form cannot express source data;
- bounded, stamped Representation Updates, correlated Interaction Outcomes,
  stale-intent rejection, and Representation Gap resynchronization;
- Undo, Redo, stale-revision conflict, cancellation, publication, and reopen;
- transactional readiness, reverse-order rollback, bounded recovery, and
  ordered shutdown;
- standard and deterministic-test Host variants;
- recorded latency, I/O, memory, and extension overhead; and
- structured, bounded, loss-visible Editor Event Stream evidence.

Deterministic public models additionally cover Read-Only plus create-another,
volatile Snapshot freshness/change gaps, and Add-Only irreversibility. They do
not silently add three production Resource Adapters or prove a shipping claim.

See [Vocabulary](../00-Vocabulary.md), the
[Architecture index](../01-Architecture/README.md),
[Resource and Buffer contract](../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md),
and [Semantics and View contract](../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md).
