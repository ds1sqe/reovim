# Meaning, interaction, and View

[Rule Matrix index](../02-Rule-Matrix.md)


| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `DOM-001` | Domain policy supplies meaning and lowers semantic intent without owning Resource connectivity or Buffer state. | [Semantic contract](../../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `STATIC,CONTRACT,INTEG` |
| `COORD-001` | Semantic coordinates and values use descriptor-qualified carriers plus exact authority and state stamps; presentation coordinates remain Client-owned. | [Semantic contract](../../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `SHAPE,CONTRACT,FAULT` |
| `REP-001` | Each Representation declares address/value model, fidelity, editability, correspondence, bounds, and oracle. | [Semantic contract](../../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `SHAPE,MODEL,GOLDEN` |
| `REP-002` | Editable high-level forms preserve unexpressed source data or reject the edit; lossy forms are read-only. | [Semantic contract](../../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `MODEL,GOLDEN,INTEG` |
| `PROJ-001` | Projection owns generation-fenced correspondence/invalidation and never becomes a second edit/history authority. | [Semantic contract](../../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `MODEL,FAULT,INTEG` |
| `INT-001` | Every Interaction Intent is authority-, identity-, revision-, version-, and generation-stamped and receives one terminal Outcome. | [Interaction contract](../../01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/01-Intent-Outcome-and-Representation-Updates.md) | `CONTRACT,FAULT,INTEG` |
| `INT-002` | Representation Updates and Interaction Outcomes remain distinct; gaps force explicit resynchronization. | [Interaction contract](../../01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/01-Intent-Outcome-and-Representation-Updates.md) | `CONTRACT,FAULT,INTEG` |
| `VIEW-001` | View/viewport/layout/input/drawing/frame policy is Client-owned and absent from Kernel contracts. | [View contract](../../01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/02-Client-View-and-Presentation.md) | `STATIC,SHAPE,INTEG` |
| `PROTO-001` | External Protocol Binding owns semantic translation while Protocol Adapter owns framing/carrier mechanics; neither redefines Reovim identity or authority. | [External protocol contract](../../01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View/03-External-Semantic-Protocols.md) | `STATIC,COMPAT,INTEG,FAULT` |
