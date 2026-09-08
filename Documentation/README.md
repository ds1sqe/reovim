# Reovim editing system

Documentation defines product goals and contracts, not implementation status.
Conceptual operations here become concrete through public API documentation,
behavior tests and a running composition. Plans own order; completed tasks
and experiment records own progress/results.

## Goal

Build a capable, extensible editing system with mission-critical quality:
preserve data, make failure and recovery explicit, then measure performance.
Text and Markdown are domains, not the shape of the Kernel.

The first concrete product target is Markdown source editing with a derived
preview, a TypeScript-ecosystem GUI and a TUI supporting Kitty graphics.
SSR is the GUI's initial presentation direction; its framework and deployment
remain open. Both clients use one editing authority with independent views.

## Interfaces

Read [architecture](architecture.md) for mechanisms, source ownership,
dependencies and Host portability. Then go directly to the owning interface:

- [Resources and state](interfaces/resources-and-state.md): open, save, rename,
  select a Buffer, edit, undo/redo, conflicts and effects.
- [Events and input](interfaces/events-and-input.md): event catalog,
  subscriptions, keymap registration and resolver behavior.
- [Lifecycle and modules](interfaces/lifecycle-and-modules.md): boot, Ready,
  capabilities, extension SDKs, unload and shutdown.
- [Clients](interfaces/clients.md): client core/contracts/modules, rendering,
  independent state and Client-Client synchronization.
- [Debugging](debugging.md): operational log, causal backtrace, state and locks.
- [Performance](performance.md): workloads, IPC, measurements and probe cost.

These focused documents replace the former numbered specification and exact
conformance registry. Readable contracts and behavior evidence remain required;
retiring the source-classifier/compiler/package proof system is not a claim
that ordinary checks provide the same automated proof.

## Release scope

v0.16 establishes the editing-system foundation. v0.17 builds Neovim feature
parity on it; plugin/runtime compatibility is not implied.

The first Markdown milestone does not remove the existing v0.16 requirement
to ship TUI, CLI and Web. Each needs its own usable Core/Contract/Module/Runtime
and production output path. Android/native support remains a future direction.

| Module SDK | Server | TUI | CLI | Web |
| --- | --- | --- | --- | --- |
| Rust | Required | Required | Required | Excluded |
| Python | Required | Optional | Optional | Excluded |
| TypeScript | Excluded | Excluded | Excluded | Required |

The matrix is a release obligation, not current availability or implementation
order. Rust establishes the public semantics first; a tested service reference
precedes Python/TS extension bindings. Developing the TS GUI does not by itself
implement its extension SDK. No embedded engine is selected.
Public v0.16 APIs remain Experimental until an explicitly admitted full SDK
stability release.

## Acceptance, not feature count

Each supported behavior needs an owning contract, a public path without
private shortcuts, bounded resources, typed failure/cancellation, lifecycle,
diagnostics and appropriate measurements. Test actual outcomes, not checkout
hashes, lengths or package counts.

First prove the executing edit/undo journey. A small non-text,
non-byte-addressed model must use the same Kernel transaction/History path.
The foundation release also retains its low/high-level fidelity proof: a
partially resident resource edited through byte/hex and Text representations,
one Buffer/History, with invalid source units preserved. Read-only/create,
volatile-source gaps and add-only effects need deterministic behavior tests.

A migrated behavior is individually adopted, adapted, rewritten or retired.
Record the donor and valid behavior, unwanted coupling, target owner,
verification and cutover in its implementation record. Old code is a behavior
donor, not design authority. Do not copy an old editor as a parallel workspace.

Qualify the exact packaged candidate on each claimed environment. A model,
compilation pass or a development crash does not qualify a running product.
Failures and unmeasured limits stay visible. Numeric performance budgets need
reproducible baselines; unobserved data is not measured zero.

Keep specifications focused and at most 250 lines each. Update the owning
contract and applicable code/tests/evidence together; keep chronology and
machine-specific results out of these documents.

[Heritage](heritage/README.md) and [past performance reports](../perf/README.md)
remain available as historical references, not current implementation proof.
