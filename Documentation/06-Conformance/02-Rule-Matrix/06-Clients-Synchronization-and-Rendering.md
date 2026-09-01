# Clients, synchronization, and rendering

[Rule Matrix index](../02-Rule-Matrix.md)


| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `CLIENT-001` | Every Client Family owns a separate Core, public Contract, Module surface, Runtime, and Presentation path. | [Client contract](../../01-Architecture/05-Client/01-Client-Ownership-and-Family-Contracts.md) | `STATIC,CONTRACT,INTEG,QUAL` |
| `CLIENT-002` | CLM owns one Client's local composition; CCM alone owns explicit cross-Client synchronization. | [Client contract](../../01-Architecture/05-Client/01-Client-Ownership-and-Family-Contracts.md) | `SHAPE,MODEL,INTEG` |
| `CCM-001` | CCM state has explicit authority/schema/privacy/bounds/gaps/resynchronization and never applies peer Buffer edits. | [Client contract](../../01-Architecture/05-Client/03-Extension-Modules-and-CCM.md) | `MODEL,FAULT,INTEG` |
| `CCM-002` | TUI, CLI, and Web prove join/leave/reconnect/gap/resync plus one mixed-family isolation journey. | [Client contract](../../01-Architecture/05-Client/04-Lifecycle-Families-and-Release-Evidence.md) | `INTEG,FAULT,PERF,QUAL` |
| `RENDER-001` | Rendering is negotiated capability, not inferred from Client Family. | [Rendering contract](../../01-Architecture/05-Client/02-Rendering-Capabilities.md) | `SHAPE,CONTRACT` |
| `RENDER-002` | Presentation Adapter alone owns concrete terminal/window/browser/GPU/device handles and loss/recreation. | [Rendering contract](../../01-Architecture/05-Client/02-Rendering-Capabilities.md) | `STATIC,FAULT,INTEG` |
| `RENDER-003` | Every selected cell/raster/pixel/video/GPU path has independent correctness and performance evidence. | [Rendering contract](../../01-Architecture/05-Client/04-Lifecycle-Families-and-Release-Evidence.md) | `GOLDEN,PERF,MEM,QUAL` |
