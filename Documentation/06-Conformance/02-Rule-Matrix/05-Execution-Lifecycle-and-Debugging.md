# Execution, lifecycle, and debugging

[Rule Matrix index](../02-Rule-Matrix.md)


| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `EXEC-001` | Each Kernel/Client Runtime owns a private non-Host ExecutionFacility with bounded admission and terminal completion. | [Execution contract](../../01-Architecture/04-Execution/03-Runtime-Execution-Facility.md) | `SHAPE,MODEL,FAULT` |
| `EXEC-002` | Invocation, completion, delivery, effect, callback, and reentrancy classes remain explicit and orthogonal; any BlockingClass alias maps only to Invocation. | [Execution contract](../../01-Architecture/04-Execution/01-Execution-Laws-and-Operation-Classes.md) | `SHAPE,CONTRACT` |
| `EXEC-003` | Buffer and Client State Lanes are separate logical linearization authorities; guest/external work never runs under them. | [Execution contract](../../01-Architecture/04-Execution/02-State-Lanes-and-Synchronization.md) | `STATIC,MODEL,FAULT` |
| `CONC-001` | No authoritative lock or State Lane is held across user, SDK, adapter, IPC, Host, presentation, or other external work. | [Execution contract](../../01-Architecture/04-Execution/02-State-Lanes-and-Synchronization.md) | `STATIC,FAULT,PERF` |
| `LIVE-001` | Progress Points, stalls, freezes, deadlocks, starvation, and livelock are typed and independently evidenced. | [Execution contract](../../01-Architecture/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md) | `MODEL,FAULT,PERF,EVENT` |
| `LIVE-002` | Production observes liveness defects without diagnostic-driven recovery mutation; qualification fails disallowed defects. | [Quality](../../07-Mission-Critical-Quality/02-Lifecycle-Isolation-and-Authority.md) | `FAULT,INTEG,QUAL` |
| `LIVE-003` | Every claimed watchdog has an observation and clock path independent of the suspected stall, lock, lane, facility, IPC, and diagnostic output. | [Execution contract](../../01-Architecture/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md) | `SHAPE,FAULT,PERF` |
| `DBG-001` | Development fatal policy captures bounded independently readable evidence before process/test termination. | [Execution contract](../../01-Architecture/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md) | `FAULT,EVENT,INTEG` |
| `DBG-002` | Reverse causal queries are bounded, authority-scoped, gap-visible, generation-exact, and non-mutating. | [Execution contract](../../01-Architecture/04-Execution/05-Causal-Debugging-and-Evidence.md) | `MODEL,FAULT,EVENT,PERF` |
