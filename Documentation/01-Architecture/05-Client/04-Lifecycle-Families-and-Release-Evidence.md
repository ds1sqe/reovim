# Client Lifecycle, Mandatory Families, and Release Evidence

[Client contract index](README.md)

## 12. Atomic pair and Client lifecycle

Assembly selects definitions, concrete adapters, supervision, and the inert
ownership graph. Assembly does not write Runtime, Module, CCM, or process
lifecycle state.

Readiness provisionally attaches:

- the process endpoint where applicable;
- Runtime-owned module contributions;
- every declared required Server/Client pair; and
- CCM membership with an authoritative Snapshot and checkpoint.

Packages declare one of server-only, client-only, paired-required, or paired-
optional with a defined degraded result.

An outside-Kernel **Pair Lifecycle Coordinator** owns only aggregate pair
readiness and outcome. It invokes public child attach, close, and rollback
operations and never writes child state. Ownership remains:

- Kernel Runtime for Server Module attachment;
- each Client Runtime for its Client Module and local state;
- the Synchronization Authority for CCM membership and state; and
- the Extension Host Supervisor or recorded external owner for process
  lifecycle.

Required-pair readiness commits only after all child predicates pass. Optional
pairing publishes only its declared degraded result. Failure advances fences and
rolls back through the owning public operations, with dependants before the
dependencies they consume, in reverse activation order.

Shutdown:

1. closes new admission;
2. advances module, Session, membership, Client, and capability fences;
3. resolves or reconciles admitted external effects;
4. drains or visibly discards bounded delivery;
5. removes contributions and CCM membership;
6. closes dependants before dependencies; and
7. lets the process owner reap only after owned execution reaches its terminal
   contract.

Restart creates new process, module, Contract Session, membership, Client,
Rendering Capability, and Presentation Adapter generations as applicable.
Prior-incarnation completion cannot install into the replacement.

## 13. Mandatory v0.16 Client Families

The mandatory v0.16 shipping Client Families are **TUI**, **CLI**, and **Web**.
The release is incomplete if any one lacks:

- an admitted exact artifact or asset entry point;
- its Family Core, Family Contract, and Family Module surface;
- at least one usable negotiated production Presentation/output composition;
- public lifecycle and failure conformance;
- its mapped real performance Scenarios;
- participation in the required common CCM composition; and
- clean-environment exact-artifact qualification.

Failure or missing evidence for one mandatory Family blocks v0.16. It cannot be
silently removed, waived, or marked not applicable to make qualification pass.
Changing this family scope requires a new owner decision.

This release requirement is a target contract. Family implementation, support,
and qualification require the separately retained evidence defined below.

### 13.1 TUI

TUI owns its terminal input and Presentation path. It must qualify at least one
selected production composition through Client-clock input-to-present.

TUI is not restricted to terminal cells. It may negotiate cell composition,
terminal raster protocols, direct raster/pixel surfaces, video or animated
frames, and GPU acceleration. Cell, Kitty/Sixel-like, raster/pixel, video, and
GPU paths are independent optional rows until separately selected. Mandatory
TUI does not make all of them mandatory.

### 13.2 CLI

CLI declares its actual command, text, byte, or structured-output capabilities
and qualifies command-to-output unless it truly owns a raw-input and
Presentation journey. CLI is not required to invent View geometry, a viewport,
frame scheduling, or GPU rendering.

CLI still has a Family Core, Contract, and Module surface. A typed no-View path
is valid when its public contract and evidence say so.

### 13.3 Web

Web qualifies through the selected real browser/runtime, exact shipped assets,
production Protocol path, selected DOM/canvas/raster/GPU Presentation
composition, browser presentation handoff, sandbox/network policy,
prerequisites, and setup/cleanup invalidators. A server result, asset build, or
synthetic browser substitute cannot qualify Web.

DOM, canvas, raster, and GPU paths are independently negotiated and evidenced.
Mandatory Web does not select all of them.

### 13.4 Future families

Android and the owner-named Native family are future. They inherit no Client,
transport, presentation, SDK, support, or qualification claim from TUI, CLI,
Web, or one another.

Adding a Family requires its own Core/Contract/Module boundary, common semantic
projection, Presentation Port and Adapter, module and CCM conformance, supported
SDK rows, performance Scenarios, and exact-artifact evidence.

## 14. Cross-family CCM release proof

The v0.16 CCM matrix includes TUI, CLI, and Web join, leave, reconnect, gap, and
resynchronization journeys against one common Synchronization Scope and schema.

At least one mixed-family composition proves that:

- one accepted shared revision is projected independently into all three
  Families' local state;
- a slow, failed, or gapped Family is isolated;
- each Family applies the authoritative Snapshot/checkpoint and predecessor
  rules;
- no rendered or Family-specific state becomes cross-family authority; and
- restart and prior-incarnation fencing preserve ownership.

The common proof does not require every pairwise permutation. A package-specific
Module Synchronization row applies only when that module/family binding is
selected and supported.

## 15. Causal debugging and performance evidence

The Causal Interaction Trace and Causal Debug Graph record applicable Client
and rendering stages using exact identities and generations, including:

- Client, Family, Module, View, layout, render, and frame;
- CCM scope, membership, state revision, queue, gap, and resynchronization;
- Rendering Capability and Presentation Adapter/target/device incarnation;
- CPU/GPU queue and synchronization stages;
- upload, readback, copy, and byte observations;
- frame generation, coalescing/drop, capability/device loss and recovery; and
- presentation handoff or typed unavailable observation.

Cross-process causality uses message/correlation identity and explicit send and
receive edges, not timestamp order. Durations from different clocks are not
added without a declared calibration covering mapping, drift, uncertainty, and
validity.

Each mandatory Family executes its own exact-package Scenario through its
Client Runtime, Client State Lane, ExecutionFacility, Module surface, actual
transport where applicable, negotiated Rendering/Presentation path, lifecycle,
fault outcomes, memory behavior, and end-to-end distribution. Shared Kernel
evidence is reusable only for causally attributed common stages.

One Family, renderer, backend, transport, language binding, aggregate, or mean
cannot qualify another or hide its failure. Optional render paths remain
separate evidence rows. Claims include their instrumentation overhead; an
unobservable stage is `unavailable`, never zero.

At minimum, every Family Scenario covers startup, readiness, attach,
representative Intent/edit/Update or typed no-View behavior, pressure,
Synchronization Gap and resynchronization, claimed recovery, and bounded
shutdown.

The exact v0.16 Release Artifact Set contains TUI, CLI, and Web artifacts or
assets and entry points, compatible contract/schema versions, required Protocol
and Presentation Adapters, dependency locks/digests, and install/bootstrap
relationships. One package set may contain all three, but omission or
substitution is not allowed.
