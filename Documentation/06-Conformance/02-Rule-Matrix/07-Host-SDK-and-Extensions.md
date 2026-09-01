# Host, SDK, and extensions

[Rule Matrix index](../02-Rule-Matrix.md)


| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `HOST-001` | Host capabilities are small, semantic, consumer-driven, and exclude platform handles/branching. | [Host API](../../02-Host-API.md) | `STATIC,SHAPE,REVIEW` |
| `HOST-002` | Initial Host scope contains only MonotonicClock and DiagnosticOutput; ExecutionFacility/engines are non-Host. | [Host API](../../02-Host-API.md) | `STATIC,CONTRACT` |
| `HOST-003` | Every Host capability has standard/test adapters, shared conformance, lifecycle, limits, events, and measurements. | [Host API](../../02-Host-API.md) | `CONTRACT,FAULT,PERF` |
| `SDK-001` | Language-neutral schemas own semantics; Rust/Python/TypeScript projections cannot redefine wire, retry, certainty, or lifecycle. | [Extensions](../../03-Extensions/02-SDKs-Service-Contracts-and-Compatibility.md) | `SHAPE,COMPAT,GOLDEN` |
| `SDK-002` | Rust-first and service-reference predecessor gates complete before Python/TypeScript admission. | [Extensions](../../03-Extensions/02-SDKs-Service-Contracts-and-Compatibility.md) | `TRACE,CONTRACT,PERF` |
| `SDK-003` | Each required or supported role/family/language row has public examples, docs, conformance, exact package, and independent evidence. | [Extensions](../../03-Extensions/04-Quick-Authoring-Admission-and-Compatibility.md) | `DOC,COMPAT,INTEG,PERF,QUAL` |
| `SDK-004` | Every public callable declares orthogonal Invocation, Completion, Delivery, Effect, Callback, and Reentrancy classes. | [Extensions](../../03-Extensions/02-SDKs-Service-Contracts-and-Compatibility.md) | `SHAPE,CONTRACT,FAULT` |
| `EXT-001` | Manifests declare Contribution Points, activation, grants, bounds, dependencies, placement, and lifecycle before code executes. | [Extensions](../../03-Extensions/01-Packages-Contribution-Points-and-Registry.md) | `SHAPE,SEC,FAULT` |
| `EXT-002` | Introspection validates Session/authority/scope before lookup and prevents metadata/pagination/continuation leakage. | [Extensions](../../03-Extensions/01-Packages-Contribution-Points-and-Registry.md) | `SEC,CONTRACT,FAULT` |
| `EXT-003` | First-party extensions use public-only paths and no private Kernel/Client shortcut. | [Extensions](../../03-Extensions/README.md) | `STATIC,INTEG` |
| `EXT-004` | Adapter and Engine descriptors separate stable identity from registry-owned scope/delivery/admission status. | [Extensions](../../03-Extensions/03-Adapters-Execution-and-Lifecycle.md) | `SHAPE,CONTRACT` |
| `EXT-005` | v0.16 Adapter Kit/TestOnly model selects no production engine or compatibility profile. | [Extensions](../../03-Extensions/03-Adapters-Execution-and-Lifecycle.md) | `STATIC,SHAPE,TRACE` |
| `EXT-006` | Quick local manifests are synthesized only before execution from explicit inputs; installed, shared, and published packages use explicit manifests. | [Extensions](../../03-Extensions/04-Quick-Authoring-Admission-and-Compatibility.md) | `SHAPE,SEC,INTEG` |
