# Domains, Representations, and Projections

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Ownership split

The Reovim Kernel owns semantic contracts and Domain-neutral coordination
mechanisms. Concrete Domain and Representation policy is injected. A Client
owns every View and all concrete presentation and interaction mechanics.

| Role | Owner | Responsibility | Does not own |
| --- | --- | --- | --- |
| Domain contract | Kernel Interface | Compatibility, semantic operation families, validation/lowering shape, bounds, and policy capability shape | Concrete data policy or presentation |
| Concrete Domain policy | Assembly-selected component or admitted Module | Parsing, navigation, commands, validation, semantic selection, and edit construction | Resource connectivity, Buffer authority, or Client layout |
| Representation contract | Kernel Interface | Interpretation, coordinate/value model, fidelity, editability, correspondence, and reverse-edit rules | Source authority or View state |
| Concrete Representation policy | Assembly-selected component or admitted Module | Forward interpretation, reverse mapping, semantic validation, and complete lowering | Buffer commit, History, or presentation mechanism |
| Projection mechanism | Kernel Core | Stamped correspondence, dependency tracking, invalidation, generation fencing, ordered bounded policy invocation, and Update publication | Concrete interpretation/lowering, independent History, rendering, or storage mutation |
| Interaction seam | Kernel Interface and Runtime | Bounded versioned Intent, correlated terminal Outcome, and Representation Update delivery | Transport encoding, device events, keybindings, or drawing |
| View | Client | Viewport, layout, clipping, scrolling, focus, local selection, hit testing, input mapping, decoration, drawing, and frame scheduling | Resource, Buffer, Transaction, or Projection authority |

A Resource Adapter connects data; it is not a Domain. A Buffer owns working
state and History; it is not a Representation. A Projection cache is derived
and never authoritative. A View is a public semantic consumer, not a Kernel
layer.

## Coordinate and value spaces

The Kernel knows a coordinate or value only through a negotiated contract,
qualified carrier, and exact state stamps. The architecture distinguishes:

| Space | Authority | Kernel treatment |
| --- | --- | --- |
| External subject reference | External Protocol Binding, such as a protocol URI or target name | Resolved under authority; never universal Resource Identity |
| Resource identity and state | Resource family Identity, Access Incarnation, and Revision | Opaque scoped stamps proving subject and observation |
| Resource Address and Unit | Resource family, such as a byte, memory address, record sequence, or native sample | Qualified by the Resource contract |
| Semantic coordinate and value | Domain/Representation, such as a text line plus encoded offset, structured node/field, or intrinsic source pixel/sample | Qualified typed/schema carrier |
| Presentation coordinate and value | Client/View, such as screen pixel, terminal cell, glyph, viewport rectangle, or pointer position | Never Kernel semantic authority |

Kernel mechanism validates the outer envelope, contract/schema identity and
version, structural bounds, authority, and state stamps. It routes, fences,
coordinates, and returns typed outcomes. Concrete Domain/Representation policy
validates field semantics. Generic Kernel code never decodes or imposes them.

## Semantic Coordinate Space Descriptor

Each addressable Domain/Representation form owns a versioned **Semantic
Coordinate Space Descriptor** declaring:

- contract/version and coordinate-space identity, plus a space incarnation when
  several live instances require one;
- coordinate, region, and value schema identities/versions;
- canonical encoding across process or persistence boundaries;
- payload, nesting, count, temporary, and owned/resident-byte bounds;
- optional equality, ordering, range, adjacency, hashing, and slicing
  operations, none inferred by Kernel mechanism;
- Unit, origin, axis, encoding, and normalization semantics;
- state stability, stale rules, and any explicit remap operation;
- invalid, out-of-scope, ambiguous, no-correspondence, stale,
  unsupported-schema, and pressure outcomes; and
- ownership/borrow/drop, lifetime, async retention, thread transfer/affinity,
  sharing/concurrency, panic/unwind, and unsafe-boundary rules.

A **Semantic Location** combines a descriptor-qualified carrier with applicable
Resource Identity/Access/Revision, Buffer identity/incarnation/version,
Representation identity, Projection generation, and coordinate-space
identity/incarnation stamps. Contract Session, request identity, and authority
belong to the enclosing Intent or request. A scoped handle may omit repeated
fields only when its validated binding supplies the same stamps. An unqualified
number has no semantic authority.

## Domain contract and concrete policy

Kernel Interface owns the Domain contract. Kernel Runtime supplies generic
admission, invocation, bound, cancellation, lifecycle, and generation-fencing
mechanism. Neither embeds one Domain's content rules.

A Domain descriptor declares the exact capabilities supplied by concrete
policy. Capability families include detection, parsing, validation, navigation,
semantic selection, commands, Representation factories, forward/reverse
mapping, and complete lowering from Interaction Intent to Buffer-valid
Transactions and Operations. A Domain need not implement the entire inventory.

Detection is advisory and inspectable. It grants no absent Resource capability
and rewrites no data. Domain policy reads public stamped contracts and submits
typed intent; it does not reach through Resource or Host Adapter internals.

No universal Domain registry, identifier allocation, dispatch table, semantic
opcode set, or language trait layout is part of this contract. Such a selection
requires a concrete consumer, target-contract change, and conformance evidence.

## Representation and fidelity

A Representation declares:

- compatible Resource and Domain capabilities and source types;
- its Semantic Coordinate Space Descriptor and output value model;
- source Revision, Buffer version, Projection generation, and source-range
  correspondence;
- editability;
- fidelity class and exact fidelity scope;
- canonical round-trip or recovery oracle;
- Interaction Intent schema and bounds; and
- how valid intent lowers completely to a Transaction.

Fidelity classes are:

- **Faithful:** every source Unit in scope is directly represented or recoverable
  through lossless correspondence, and the canonical no-op oracle reproduces the
  exact scoped Units.
- **Constrained editable:** declared constructs are exposed, while every
  accepted edit preserves unexpressed source state or rejects.
- **Derived read-only:** the form may summarize or lose information and cannot
  directly create edits.

Destructive normalization, when supported, is an explicit Transaction rather
than a viewing or round-trip side effect. A lossy form does not become editable
by inventing source state.

Low-level and high-level describe interpretation, not authority. A native-Unit
or hexadecimal form and decoded Text or structured form may observe one Buffer.
Text remains one Domain; strings, lines, graphemes, encoding conventions,
terminal cells, and presentation pixels do not enter neutral Resource or Buffer
contracts.

## Correspondence and source/presentation separation

Concrete Domain/Representation policy computes bounded state-stamped
correspondence between semantic coordinates and Resource/Buffer dependencies.
It declares cardinality, fidelity, reverse mapping, preservation, and typed
stale, ambiguous, incomplete, and no-mapping outcomes.

Kernel Core retains and fences generic correspondence but cannot inspect a
concrete `line`, `node_id`, `x`, `y`, or equivalent field.

An intrinsic image pixel may be Resource data or a semantic image coordinate.
It may appear in a schema-qualified Representation Update. A target-screen
pixel, viewport transform, draw list, presentation surface, or frame instruction
remains Client/Presentation-private.

## Projection mechanism

A Projection is the revision-aware synchronization instance for one
Representation over one Buffer. Kernel Core records exact Resource Revision,
Buffer identity/incarnation/version, Projection generation, bounded source
dependencies and correspondence, and state required to reject stale work.

When a Transaction is accepted, affected generations become stale before an
older result can install as current. Old work is cancelled, rejected, or
recomputed. Every Representation descriptor declares observable output,
correspondence, invalidation/diagnostic fields, typed outcomes, and a
differential oracle. Full and incremental computation produce equivalent
declared observables under that oracle.

Kernel Core owns the generic state machine and invokes policy through a bounded
public hook. Concrete policy owns forward computation, reverse mapping,
validation, and lowering. It returns fully lowered Operations and Preconditions
before atomic Buffer acceptance. Projection is not a replaceable mutation
executor.

No invalidation graph, cache, scheduler, diff algorithm, or computation
placement is selected here. Concrete Text, ELF, QEMU, image, or structured-data
semantics remain policy.
