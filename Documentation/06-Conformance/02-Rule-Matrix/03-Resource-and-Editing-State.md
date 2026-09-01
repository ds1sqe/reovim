# Resource and editing state

[Rule Matrix index](../02-Rule-Matrix.md)


| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `RES-001` | Resource identity, Address, Extent, Unit, Revision, capabilities, limits, and errors are explicit and Resource-native. | [Resource contract](../../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `SHAPE,CONTRACT` |
| `RES-002` | Resource capabilities compose independently; unsupported behavior is typed and never inferred. | [Resource contract](../../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `SHAPE,CONTRACT,FAULT` |
| `RES-003` | Open establishes authority, coherent initial Snapshot/watch state, and one live Access Incarnation atomically. | [Resource contract](../../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `MODEL,CONTRACT,FAULT` |
| `RES-004` | Snapshot consistency, capture/freshness/time authority, structure, limits, and gaps are explicit. | [Resource contract](../../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `SHAPE,MODEL,FAULT` |
| `RES-005` | External change notification/feed and initial snapshot/subscription handoff are race-free and gap-visible. | [Resource contract](../../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `MODEL,CONTRACT,FAULT` |
| `CREATE-001` | Create-another uses an independently authorized immutable Resource Creation Intent without mutating or rebinding its source. | [Resource contract](../../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `MODEL,FAULT,IO,INTEG` |
| `ADD-001` | Add-Only uses a Resource-native monotonic relation; Undo never misstates an applied external addition as rollback. | [Resource contract](../../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `MODEL,CONTRACT,FAULT` |
| `BUF-001` | One Buffer is transactional working state for exactly one Resource and editing context. | [Buffer contract](../../01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md) | `SHAPE,MODEL,INTEG` |
| `BUF-002` | Opening and editing support bounded partial residency without default full-resource cloning/materialization. | [Buffer contract](../../01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md) | `MODEL,MEM,IO,PERF` |
| `TXN-001` | Transactions validate Preconditions and atomically accept or reject fully lowered Buffer-valid Operations. | [Transaction contract](../../01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md) | `MODEL,CONTRACT,FAULT` |
| `TXN-002` | History/Undo/Redo preserve coherent reconstructable accepted state within declared bounds. | [Transaction contract](../../01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md) | `MODEL,FAULT,MEM` |
| `PUB-001` | Local acceptance and external publication are separate outcomes. | [Publication contract](../../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `CONTRACT,FAULT,INTEG` |
| `PUB-002` | Effect Certainty and Outcome Cause remain orthogonal through cancellation, timeout, failure, and response loss. | [Publication contract](../../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `MODEL,CONTRACT,FAULT` |
| `PUB-003` | Outcome-unknown publication retains a bounded public reconciliation owner/witness and prevents blind retry. | [Publication contract](../../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `MODEL,FAULT,IO` |
| `FENCE-001` | Resource creation/access, Buffer, Projection, Contract/Protocol Session, IPC, Client/Family/Module/render/CCM, Host, adapter, and engine identities/generations fence stale work at every listed cut. | [Quality fence matrix](../../07-Mission-Critical-Quality/01-Integrity-Effects-and-Fencing.md) | `MODEL,FAULT,INTEG` |
