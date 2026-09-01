# Client Ownership and Family Contracts

[Client contract index](README.md)

## 2. Client ownership boundary

A Client owns every applicable presentation/output state it instantiates. For a
View-bearing path, this includes:

- persistent cursor and selection state unless separately promoted to shared
  semantic state;
- focus, scroll, viewport, visible-range, clipping, and hit-testing policy;
- layout and presentation-local mode;
- device input and keybinding mapping;
- View, decoration, drawing, theme, render, and frame policy; and
- presentation scheduling and target handoff.

A typed no-View path, such as an honest CLI composition, instead owns its
declared bounded output state and terminal handoff. It does not create
viewport, layout, cursor, drawing, or frame state merely to satisfy the Family
contract.

Before submitting an Interaction Intent, the Client maps any device or
presentation coordinate to a qualified stamped semantic coordinate. Carrying a
semantic location in one request does not move persistent View state into the
Kernel.

Shared, persistent, collaborative, or command-semantic state moves out of the
Client-local default only through a separately admitted contract with explicit
authority, identity/version, bounds, lifecycle, fencing, privacy, failure, and
conformance.

Client state installation uses the Client State Lane defined in
[Execution and Concurrency](../04-Execution/02-State-Lanes-and-Synchronization.md).
It never acquires
Buffer mutation authority.

## 3. CLM and CCM are separate models

The **Client Layer Model (CLM)** defines composition, extension, and
presentation within one Client Runtime. CLM owns no cross-Client
synchronization authority.

The **Client–Client Model (CCM)** defines versioned synchronization among two
or more Client instances. CCM owns no:

- View, viewport, layout, drawing, frame, or presentation policy;
- Resource, Buffer, History, Transaction, or editing authority; or
- transport, carrier, process, or connection mechanics.

CLM and CCM may exchange only values admitted by the common Client Semantic
Contract or a separately admitted synchronization schema. Similar field names,
matching numeric generations, or co-location in one process do not merge their
authorities.

Historical CLM layouts, traits, FFI, single-task assumptions, Client Buffer
caches, untyped pairing, and fire-and-forget handles are donor material only.
This chapter makes no compatibility claim with them.

## 4. Client Family: Core, Contract, and Module

A **Client Family** is one complete Client implementation family. Every family
has three distinct surfaces.

### 4.1 Family Core

The **Family Core** is family-local mechanism for:

- adapting public Client contracts;
- composing and dispatching modules;
- coordinating Client-local state;
- enforcing Family Contract bounds and generations;
- family readiness, rollback, supervision, and shutdown; and
- using its Client Runtime's private ExecutionFacility.

The Family Core knows no concrete feature module and exposes no private Kernel,
Runtime, state-lane, Protocol Adapter, or Presentation Adapter path.

### 4.2 Family Contract

The **Family Contract** is a public, versioned capability and module-
contribution contract. It owns only that family's:

- input and local interaction surface;
- View and layout contribution schemas;
- explicit Rendering Capabilities;
- presentation or output semantics;
- lifecycle, bounds, pressure, and failure behavior; and
- family-specific compatibility rules.

A Family Contract specializes Client-side behavior. It may not redefine
Resource, Buffer, History, Transaction, Domain, Representation, Projection,
Interaction Intent/Outcome, or Effect Certainty semantics.

### 4.3 Family Module surface

The **Family Module surface** is the public extension surface through which
feature policy implements a Family Contract. A family module depends only on
public common Client semantics and its Family Contract. It does not depend on:

- Family Core internals;
- another Client Family;
- private Kernel Core or Runtime state; or
- concrete Protocol or Presentation Adapters.

For every mandatory shipping Family, the Module surface includes released
extension-point contracts, attachment and lifecycle behavior, SDK/support-
registry projection, a public example or conformance fixture, failure
containment, and mapped qualification evidence.

The required Module surface does not require every Client instance to load a
feature module or require a built-in module unless a separately admitted
product profile names one.

## 5. Common Client Semantic Contract

The **Client Semantic Contract** is the one cross-family authority for:

- Client and Contract Session identity;
- Interaction Intent and terminal Interaction Outcome;
- stamped Representation Updates;
- qualified semantic carriers;
- ordering, cancellation, and Effect Certainty;
- delivery gaps and resynchronization; and
- compatibility.

There is no authoritative global Common Client Core. Families may share
neutral libraries, generated schema types, fixtures, and helpers, but shared
implementation owns no Client lifecycle or state.

One Client Runtime instance selects exactly one Client Family and owns:

- that Family Core;
- its public Contract Sessions;
- one Client Module Set;
- its Client-local state and Client State Lane;
- its View and Presentation subtree; and
- its private ExecutionFacility.

A Family Contract cannot invent a private editing path, restamp common semantic
state, or treat a cached Representation/CCM value as authority.

## 6. Client Module Set and instance isolation

Every Client Runtime exposes its accepted Family Extension Point set and owns
one **Client Module Set**, which may be empty at runtime.

A Client Module Definition targets one Family unless its package contains
separate definitions for several Families. Each Client Module Instance binds to
exactly one Client identity/incarnation, one Family, one Contract Session, and
one Client Runtime subtree.

Cohosted instances retain separate:

- identities and incarnations;
- grants and authority generations;
- queues, byte/work charges, and concurrency bounds;
- Client and module state generations;
- failure and isolation records; and
- shutdown ownership.

One instance never imports another Family's module or obtains authority merely
because they share a package, process, language host, transport, or schema
implementation.
