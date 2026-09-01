# Evidence Taxonomy and Authority

[Conformance requirements index](01-Requirements-and-Evidence.md)

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

## Evidence scope and exceptions

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
