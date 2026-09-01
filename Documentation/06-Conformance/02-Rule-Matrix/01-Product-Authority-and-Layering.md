# Product, authority, and layering

[Rule Matrix index](../02-Rule-Matrix.md)


| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `PROD-001` | Reovim is a text-neutral editing system; Kernel never means an OS product. | [Concept](../../00-Concept/01-Product-System-and-Kernel.md) | `DOC,STATIC,TRACE` |
| `PROD-002` | v0.16 establishes the foundation; v0.17 parity uses v0.16 public contracts. | [Documentation authority](../../README.md) | `DOC,TRACE,REVIEW` |
| `PROD-003` | TUI, CLI, and Web are mandatory v0.16 Client Families; none is waived as not-applicable. | [Quality](../../07-Mission-Critical-Quality/03-Admission-Qualification-and-Release-Scope.md) | `TRACE,INTEG,QUAL` |
| `PROD-004` | The official SDK role/family BindingScope and DeliveryGate matrix is exact and independently evidenced. | [Extensions](../../03-Extensions/01-Packages-Contribution-Points-and-Registry.md) | `SHAPE,COMPAT,INTEG,QUAL` |
| `AUTH-001` | Normative Documentation, plans, trackers, and evidence retain distinct authority. | [Requirements authority](../01-Requirements-and-Evidence.md) | `DOC,TRACE` |
| `ARCH-001` | Product dependencies point inward without cycles; Repository Tooling grants no product edge and enters no product shipping closure. | [Dependency graph](../../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/03-Source-Dependency-Graph-and-Edges.md) | `STATIC,TRACE` |
| `ARCH-002` | Foundation and Kernel contracts contain no concrete environment, adapter, Domain, transport, Client, presentation, or extension mechanism. | [Dependency graph](../../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/03-Source-Dependency-Graph-and-Edges.md) | `STATIC,SHAPE` |
| `ARCH-003` | Resource, Host, Protocol, Presentation, Extension Adapter, and Extension Engine Adapter roles remain distinct. | [Roles](../../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/01-Logical-Roles-and-Source-Ownership.md) | `STATIC,INTEG` |
| `ARCH-004` | Assembly selects an inert graph; each subtree has one lifecycle authority and no hidden global discovery. | [Layer model](../../01-Architecture/01-Layer/02-Interaction-Assembly-and-Lifecycle.md) | `SHAPE,FAULT,INTEG` |
| `ARCH-005` | Every resolved third-party Cargo package has an exact machine admission with metadata-resolved consumers/profiles and retained review; every other dependency mechanism has its own exact pin/admission and retained review. | [Dependency admission](../../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies/02-Descriptors-and-Dependency-Admission.md) | `STATIC,REVIEW,PERF` |
| `ARCH-006` | Every evidence set carries an exact-revision Source Role Map generated from separate schema-valid classification policy, exact workspace topology, entrypoint reviews, and dependency admissions. Phase-B v1 reports only all-RepositoryTooling dependency reachability and fails closed on Product/shipping input. | [Classification artifacts](../04-Source-Classification-and-Exact-Checker/01-Policy-Artifacts-and-Source-Axes.md) | `SHAPE,STATIC,TRACE,REVIEW` |

Phase-B v1 does not satisfy product applicability of `ARCH-001`; its positive
scope is the exact all-`RepositoryTooling` topology only.
