# Editing system design

Start with [one editing journey](00-overview.md). Chapters 01–05 introduce the
contracts in dependency order. Read the relevant worked examples in 07 alongside
them; callers need not read the native records first. Provider and adapter
implementers use 06 as the declaration/ABI reference.

These are target contracts. Source and independent provider/consumer execution
establish support; [product goals](../README.md) own release scope. Rust snippets
show signatures or SDK use with stated inputs; they are not complete programs.
Chapter 09 records future authoring directions with explicitly open interfaces.

## 1. Resources and calls

- [Resource identity and opened access](01-resources.md)
- [Namespace and naming operations](01.1-namespace.md)
- [Operation selection, binding and outcomes](01.2-operations.md)
- [Request custody and recovery](01.3-requests.md)
- [I/O transfer semantics](01.4-io.md)
- [Resource metadata](01.5-metadata.md)

## 2. Editable state

- [Buffer state, atomic editing, snapshots and close](02-buffer.md)
- [History, Undo and Redo](02.1-history.md)
- [Source attachments and capture](02.2-sources.md)
- [Save, export and Save As](02.3-publication.md)

## 3. Observation and interpretation

- [Watches, baselines, gaps and closure](03-observation.md)
- [Representation and qualified semantic values](03.1-representation.md)

## 4. Clients and retained context

- [Input, presentation and Client extensions](04-clients.md)
- [Sessions, environment and participation](04.1-session.md)

## 5. Execution and composition

- [Runtime driving and lifecycle](05-runtime.md)
- [Dependency direction and provider wiring](05.1-composition.md)
- [Kernel modules and contribution roles](05.2-modules.md)
- [Module registry, modprobe and unload](05.3-modprobe.md)

## 6. Declaration and ABI reference

- [API declarations and canonical values](06-declarations.md)
- [Native ABI and service boundaries](06.1-native-abi.md)
- [Native registration and pending requests](06.2-native-dispatch.md)
- [State, factory and preparation records](06.3-state-providers.md)
- [Opened I/O, namespace and watch encoding](06.4-io-providers.md)
- [Caller records and service operation bodies](06.5-caller-protocol.md)

## 7. Worked examples

- [Extension data APIs, Parse and typed edits](07-domain-apis.md)
- [Live I/O, capture and explicit output](07.1-live-io.md)
- [Assembly, modules, Sessions and two Clients](07.2-module-journey.md)
- [Navigation, Vim/Emacs commands and motion help](07.3-navigation.md)
- [Language services and peer observation](07.4-service-workflows.md)
- [Optional Client Resource exports](07.5-client-resources.md)
- [A .map file, 3D movement and zoom across layers](07.6-map-journey.md)

## 8. Behavioral references

- [External influences and their implications](08-references.md)

## 9. Future directions

- [Scope and connections to existing contracts](09-future.md)
- [Client modules, schema exports and execution](09.1-client-modules.md)
- [User configuration, settings and input bindings](09.2-user-configuration.md)

[Debugging](../debugging.md) and [performance](../performance.md) own diagnostics
and measurement. Each chapter owns the rules named above; other chapters link
to that owner. Execution order, implementation status and review records belong
in plans, not additional design recaps.
