# Conformance Requirements and Evidence

## Purpose and authority

Normative Documentation defines stable obligations and evidence requirements.
Plans define execution order. Flight logs record chronological progress.
GitHub records public work state. Benchmark and test artifacts retain results.
None of those temporal facts belongs in the normative contract merely because
it is current when the contract is written.

A document, type name, passing narrow test, or historical implementation is
not by itself conformance evidence.

Normative target language does not imply current implementation. Current
physical dependency truth comes from build metadata and generated or retained
evidence; implementation status comes from code plus matching conformance
results; execution chronology remains in plans and logs. Two documents cannot
grant conflicting authority over the same question.

## Conformance model

Each normative obligation receives one stable rule identifier, an
applicability statement, a single responsible boundary, and the verification
classes that can prove it. Every normative `MUST` or `MUST NOT` maps to a
matrix row; every row links back to its defining contract. The requirements
index must have zero orphan requirements and zero rows without an owning
contract. Rule identifiers are not copied automatically from the discarded
specification or stopped prototype.

The requirements index uses one versioned, closed verification taxonomy. The
initial taxonomy identity is **`reovim-evidence/v1`**. Every Rule Matrix
revision declares exactly one taxonomy identity; a row may name only codes in
that version. Adding, removing, or changing a code's meaning creates a new
taxonomy version and requires explicit migration of affected rows. The v1
codes are:

- `DOC`, `TRACE`, `STATIC`, `SHAPE`, `UNIT`, `MODEL`, `CONTRACT`, `FAULT`,
  `INTEG`, and `GOLDEN`;
- `COMPAT`, `PERF`, `MEM`, `IO`, and `EVENT`; and
- `SEC`, `REVIEW`, and `QUAL`.

Each code has one definition in the requirements index. A requirement selects
only the classes that can prove its own scope; the taxonomy does not imply that
every class applies to every rule.

| Code | Evidence definition |
| --- | --- |
| `DOC` | Documentation structure, links, terminology, and authority checks |
| `TRACE` | Requirement-to-source-to-artifact traceability |
| `STATIC` | Dependency, import, public-type, or build-metadata boundary check |
| `SHAPE` | Descriptor, schema, record, workload, or result-shape validation |
| `UNIT` | Focused deterministic behavior |
| `MODEL` | Model, property, state-machine, or differential proof |
| `CONTRACT` | Shared public contract suite across implementations |
| `FAULT` | Deterministic failure, pressure, timing, cancellation, or lifecycle injection |
| `INTEG` | Composed public-boundary journey |
| `GOLDEN` | Canonical byte, schema, rendering, or output fixture |
| `COMPAT` | Cross-version, transport, or language compatibility |
| `PERF` | Latency, throughput, or overhead Experiment Bundle |
| `MEM` | Allocation, residency, charged-byte, or process-memory artifact |
| `IO` | I/O operation, byte, durability, or reconciliation artifact |
| `EVENT` | Event, causal, redaction, correlation, and loss proof |
| `SEC` | Authority, authentication, trust, and exposure proof |
| `REVIEW` | Engineering, dependency, unsafe, migration, or evidence review |
| `QUAL` | Exact packaged-artifact qualification in a clean claimed environment |

The Layer Model owns role and dependency semantics. This Conformance contract
owns the schema for one versioned **Source Role Map** evidence artifact. Every
implementation or release evidence set publishes it under the stable artifact
key `conformance/source-role-map.json`; this key names a bundle entry, not a
claim that the current source tree implements it. The artifact records schema
identity/version, source revision, package/build graph identity, ordered path
rules, role owner, generated/vendored/test/example/tool/build-output treatment,
overlap precedence, unmatched-path policy, and the checker identity/result.

An unmatched or multiply classified path without an explicit precedence result
is a conformance error. Cargo/build metadata is the dependency-edge authority
and is checked against that map; prose cannot grant an exception to an edge
that the metadata or checker forbids. Both the package graph and its projection
onto architectural owners remain acyclic. Temporary exceptions live in
explicit tracker/evidence records and never become an unreported allowlist or
a second edge registry.

Crate-level edges are not sufficient proof. Transitive-closure, source/import,
and public-type
checks reject direct operating-environment, filesystem, socket, protocol,
codec, input-device, terminal, renderer, and platform types inside portable
Foundation and Kernel packages even when the containing crate has an otherwise
allowed name.

The evidence scope must match the claim. A component is conforming only when
all applicable obligations have current reproducible evidence. Implementation
reports may distinguish planned, partial, conforming, and justified
not-applicable states, but the normative matrix contains no live component
status.

Temporary exceptions do not weaken a rule. They require an explicit owner,
rationale, bounded effect, acceptance condition, and removal milestone in the
tracker and evidence record.

An exception is development-only and release-disqualifying. Before a slice or
release is called conforming, every applicable exception is removed or the
normative requirement itself is explicitly revised through its authority and
the resulting evidence is regenerated.

## Public reduction and Client proof

First-party Clients, CLI surfaces, adapters, and extensions cross the same
supported contracts as third parties of the same role. If valid behavior
cannot be reduced to public Interaction Intents, Transactions, Interaction
Outcomes, Representation Updates, Resource/Host contracts, and Event Stream
operations, the contract is reviewed; a private Buffer or concrete-adapter
bypass is not added.

The public Client journey proves input mapping to semantic intent, Kernel
state transition, correlated terminal outcome, bounded revision/incarnation/
generation-stamped update, Representation Gap/resynchronization,
client-owned viewport/layout/render, presentation handoff, and ordered
shutdown. Dependency and public-type checks prove no viewport, terminal cell,
target-screen or presentation pixel, renderer, raw input device, transport, or
concrete adapter type enters the Kernel. Schema-qualified intrinsic source
pixels remain permitted semantic values under the Representation contract.


## Release audit and qualification

A release claim is checked requirement by requirement against the
normative matrix and retained evidence. Missing, indirect, stale, or narrower
evidence leaves the obligation unproved. The audit also verifies that no
active OS product/runtime path, legacy system-kernel dependency, universal
`no_std` rule, text- or viewport-shaped neutral contract, duplicate editing
authority, unbounded diagnostic path, or private extension/Client dependency
remains.

Implementation conformance may be claimed for a component only from its
applicable committed code and matching evidence. Before a release is described
as qualified, the exact release artifact—not an adjacent source checkout—runs
its named public journeys and conformance subset
in clean supported environments with recorded tool/runtime prerequisites,
target identity, results, and artifacts. Qualification states only the tested
scope and does not imply a regulated certification.

The mission-critical feature gate and lifecycle obligations are developed in
[Mission-Critical Quality and Release Qualification](../07-Mission-Critical-Quality/README.md).
This contract contains no migration inventory, implementation status,
schedule, checkpoint, or completed conformance claim.
