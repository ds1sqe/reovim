# Cross-cutting safety, bounds, effects, and exposure

[Rule Matrix index](../02-Rule-Matrix.md)


| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `SAFE-001` | Every opaque carrier has an enforced Carrier Safety Descriptor covering ownership/lifetime, transfer, sharing, panic/unwind, and unsafe boundaries. | [Descriptors](../../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/02-Descriptors-and-Dependency-Admission.md) | `SHAPE,STATIC,FAULT` |
| `BOUND-001` | Every bounded capability/operation has one Bound Descriptor declaring units, per-object/aggregate maxima, checked arithmetic/overflow/zero semantics, pre-allocation validation order, charge ownership, and typed exhaustion. | [Descriptors](../../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/02-Descriptors-and-Dependency-Admission.md) | `SHAPE,MODEL,FAULT` |
| `ISOLATE-001` | Every payload, retained state, work unit, queue, concurrency group, retry, callback, drain, and wait is aggregate-bounded, charged, pressure-visible, deadline-bounded where applicable, and isolated from unrelated consumers. | [Quality](../../07-Mission-Critical-Quality/02-Lifecycle-Isolation-and-Authority.md) | `MODEL,FAULT,MEM,PERF` |
| `EFFECT-001` | Every public external mutation or operation relaying its result preserves Effect Certainty and Outcome Cause through cancellation, timeout, response loss, shutdown, and reconciliation. | [Quality](../../07-Mission-Critical-Quality/01-Integrity-Effects-and-Fencing.md) | `MODEL,CONTRACT,FAULT,IO` |
| `SEC-001` | Authority precedes lookup/exposure; protected metadata, network/native access, and unclassified behavior fail closed with bounded redaction and revocation. | [Quality](../../07-Mission-Critical-Quality/02-Lifecycle-Isolation-and-Authority.md) | `SHAPE,SEC,FAULT` |
| `PUBLIC-001` | First-party Clients, CLI paths, adapters, and extensions reduce through the same supported public contracts as equivalent third parties, without private state shortcuts. | [Public reduction](../05-Public-Reduction-and-Qualification.md) | `STATIC,CONTRACT,INTEG` |
| `IR-001` | A general Editing IR or pass manager is admitted only after two independent implemented consumers prove the same Transaction gap, measured boundary costs, and Transaction Contract Authority approval. | [Layer model](../../01-Architecture/01-Layer/02-Interaction-Assembly-and-Lifecycle.md) | `TRACE,INTEG,PERF,REVIEW` |
