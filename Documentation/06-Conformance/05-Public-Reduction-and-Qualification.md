# Public Reduction and Qualification

[Conformance requirements index](01-Requirements-and-Evidence.md)

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
