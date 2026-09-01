# Architecture Contracts

> **Normative target contract.** This index routes the v0.16 architecture
> contract. Present-tense requirements in the linked chapters define the target;
> they do not assert current implementation or qualification.

## Authority

The chapters below collectively own Reovim's architecture. This index defines
reading order and routes ownership; it does not duplicate or weaken their
requirements. The Rule Matrix links each obligation to the exact chapter that
owns it.

Read the Layer and Dependency chapters before changing an architectural
boundary, then read every chapter owning the affected contracts. Adjacent Host,
extension, performance, Event, migration, conformance, and quality contracts
remain authoritative for their own subjects.

## Layer and dependency model

1. [Roles, Descriptors, and Dependencies](01-Layer/01-Roles-Descriptors-and-Dependencies.md)
   routes focused chapters for logical/source ownership, carrier and bound
   discipline, dependency admission, and allowed and forbidden source edges.
2. [Interaction, Assembly, and Lifecycle](01-Layer/02-Interaction-Assembly-and-Lifecycle.md)
   owns the Client interaction seam, Assembly selection, readiness, rollback,
   shutdown, runtime relationships, and deliberately unselected mechanisms.

Applicable Rule Matrix families: `ARCH`, `SAFE`, `BOUND`, and `IR` as routed by
the individual rows.

## Resource and Buffer

1. [Resource Access, Snapshots, and Change](02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md)
   owns Resource and Resource Adapter capabilities, selection, open, Snapshot,
   freshness, and change discovery.
2. [Buffer, Transactions, History, and Residency](02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md)
   owns Buffer authority, Transactions, History, stale-work fencing, and partial
   residency.
3. [External Effects and Conformance](02-Resource-and-Buffer/03-External-Effects-and-Conformance.md)
   owns create-another, Add-Only external effects, publication, reconciliation,
   fidelity, and proof obligations.

Applicable Rule Matrix families: `RES`, `CREATE`, `ADD`, `BUF`, `TXN`, and
`PUB` as routed by the individual rows.

## Semantics and Client-owned View

1. [Domains, Representations, and Projections](03-Semantics-and-View/01-Domains-Representations-and-Projections.md)
   owns semantic coordinate/value spaces, Domain and Representation policy,
   fidelity, correspondence, and Projection.
2. [Interaction, Updates, Protocols, and View](03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md)
   owns Interaction Intent/Outcome, Representation Update, Client-owned View,
   external semantic protocols, lifecycle, and end-to-end proof.

Applicable Rule Matrix families: `DOM`, `COORD`, `REP`, `PROJ`, `INT`, `VIEW`,
and `PROTO` as routed by the individual rows.

## Execution and concurrency

1. [Execution Laws and Operation Classes](04-Execution/01-Execution-Laws-and-Operation-Classes.md)
   owns execution scope, laws, and orthogonal operation classifications.
2. [State Lanes and Synchronization](04-Execution/02-State-Lanes-and-Synchronization.md)
   owns Buffer and Client State Lanes, synchronization, and reentrancy.
3. [Runtime ExecutionFacility](04-Execution/03-Runtime-Execution-Facility.md)
   owns the private per-Runtime facility, admission, realization, ownership,
   failure, and shutdown contract.
4. [Liveness, Watchdog, and Fail-Fast](04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md)
   owns progress/liveness classifications, watchdog observation, diagnostic
   snapshots, production policy, and development fatal behavior.
5. [Causal Debugging and Evidence](04-Execution/05-Causal-Debugging-and-Evidence.md)
   owns the causal relation, reverse-debug graph/query, retention/gaps, and
   execution evidence obligations.

Applicable Rule Matrix families: `EXEC`, `CONC`, `LIVE`, and `DBG` as routed by
the individual rows.

## Client, CLM, CCM, and rendering

[Client, CLM, CCM, and Rendering](05-Client/README.md)
owns the Client Family, synchronization, Module, rendering, and presentation
contracts. It remains a sibling architecture contract.

## Adjacent normative contracts

- [Host API](../02-Host-API.md)
- [Extension Contracts](../03-Extensions/README.md)
- [Performance](../04-Performance/README.md)
- [Editor Event Stream](../05-Editor-Event-Stream/README.md)
- [Selective Migration](../05-Migration/01-Selective-Migration.md)
- [Conformance Requirements and Evidence](../06-Conformance/01-Requirements-and-Evidence.md)
- [Rule Matrix](../06-Conformance/02-Rule-Matrix.md)
- [Mission-Critical Quality](../07-Mission-Critical-Quality/README.md)
