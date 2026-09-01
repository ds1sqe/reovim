# Product, System Model, and Kernel

> **Normative concept chapter.** Part of the
> [Editing-System Concept](../00-Concept.md).

## North Star

Reovim is a capable, high-performance, extensible editing system pursuing
mission-critical quality for inspecting and modifying addressable Resources.
A Resource can appear in synchronized faithful and domain-aware forms: native
units, a hexadecimal layout, text, decoded records, executable metadata,
intrinsic media, or another interpretation supplied by a Domain.

Moving between forms does not create competing authorities over the data.
Editable forms intended to share working state contribute to one Buffer and one
History. A Resource may still have explicitly distinct Buffers for explicitly
distinct editing contexts.

## Product obligations

The Editing System:

1. preserves source fidelity, including Units a high-level Domain cannot
   interpret;
2. prevents silent corruption and makes conflict, Effect Certainty, recovery,
   cancellation, pressure, and shutdown explicit;
3. lets Domains add meaning without placing their coordinate systems or policy
   in neutral contracts;
4. synchronizes low-level and high-level edits through explicit Projections and
   Transactions;
5. supports bounded partial residency instead of requiring whole-Resource
   materialization by default;
6. exposes small, versioned, bounded, inspectable, and testable extension,
   Resource, Host, protocol, and presentation boundaries; and
7. makes reaction latency, I/O, memory growth, extension cost, diagnostic cost,
   and exact release qualification reproducibly measurable.

## System model

~~~text
device input                                             terminal / GUI / web
     |                                                            ^
     v                                                            |
Client input mapping -> typed Interaction Intent                  |
                              |                                   |
                              v                                   |
              +----------------------------------------+          |
              |             Reovim Kernel              |          |
              | Domain / Representation lowering       |          |
              | -> Transaction / Operations            |          |
              | -> Buffer / History / Projection       |          |
              +----------------------------------------+          |
                              |                                   |
                              +-> bounded Representation Update --+
                              +-> typed Outcome / structured Event
                              ^
                              | Resource / Host contracts
                              |
             concrete adapters selected and injected by Assembly
                    ^                              ^
                    |                              |
       Resource-native subject             process environment
~~~

The arrows show contract use and data flow, not a required object layout.

- The Reovim Kernel owns portable, Domain-neutral contracts and generic
  coordination mechanisms for revision-bound state transitions and
  Representation Updates.
- A Client owns its applicable presentation/output state. Any View, viewport,
  layout, focus, input mapping, drawing, or frame state it instantiates is
  Client-owned. It submits typed Interaction Intents against the state it
  observed; it does not mutate Buffer or concrete storage directly.
- A Resource Adapter connects a Resource-native subject to the Resource
  contract. A Host Adapter implements a semantic Host Capability using the
  process environment. The roles remain distinct even when one consumes a
  capability supplied by the other.
- A Protocol Adapter owns encoding, framing, carrier, and connection mechanics.
  An External Protocol Binding, when present, owns semantic translation between
  an external protocol and public Reovim contracts.
- Assembly selects and constructs an inert graph of concrete adapters, Clients,
  Domains, Representations, extension components, and presentation components.
  Named Lifecycle Owners activate and close their subtrees. The Kernel does not
  discover components through hidden globals.

## Reovim Kernel

The **Reovim Kernel** is the portable execution and lowering substrate of the
Editing System. It owns the public contracts and generic mechanisms needed to
turn revision-bound editing intent into coherent Buffer transitions and
revision-bound Representation Updates.

The managed-runtime/compiler-substrate analogy is limited to role:

- the Kernel supplies portable lifecycle, capability, isolation, and
  observation mechanisms over concrete hosts; and
- it supplies a common typed lowering boundary between higher-level intent and
  target-specific Resource effects.

The analogy does not prescribe bytecode, a JIT, a universal opcode set,
compiler passes, garbage collection, or machine-code generation. Typed
Transactions containing Operations and Preconditions are the lowered edit
form. Domain and Representation policy fully lower semantic intent into
Buffer-valid Operations before atomic local acceptance. Projection supplies
stamped correspondence and invalidation; it is not a second edit executor.

A separate Editing IR enters the architecture only after at least two
independently implemented consumers demonstrate the same documented semantic
gap in Transaction, the relevant transformation and boundary costs are
measured, and the Transaction Contract Authority accepts the abstraction.

The Kernel owns identity, revision, capability, bound, error, cancellation,
lifecycle, Resource, Snapshot, Buffer, Transaction, History, Domain,
Representation, Projection, Host, service-contract, Event, and measurement
semantics. Owning a contract does not mean embedding its concrete
implementation.

The Kernel does not own a system-services product, concrete Resource or Host
Adapters, transport or codec mechanisms, concrete viewport or layout policy,
terminal cells, presentation pixels, glyphs, rendering, raw-device mapping,
keybinding policy, concrete Domain policy, or Assembly-time component
selection.

## Neutral foundation

Neutral Kernel contracts are not defined by strings, UTF-8, lines, columns,
graphemes, text cursors, terminal cells, file paths, or one presentation
protocol. They do not assume that every Resource uses bytes, integer offsets,
stable random access, mutation, resize, or change notification.

Those properties belong to explicit Resource capabilities, concrete adapters,
Domains, Representations, and Client-owned Views. Text is a Domain. A
byte-addressed file is one useful Resource kind, not the universal model.

Environment mechanics remain outside the Kernel and enter only through
consumer-justified Host Capabilities. Host is not a registry of every service
an environment might offer.
