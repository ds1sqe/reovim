# Portable Host and `no_std` Profiles

[Ports, source tree, and portability](../03-Ports-Source-Tree-and-Portability.md)

## Portability goal

The portable Reovim contract permits an external custom OS or runtime
environment to host the Editing System by satisfying declared prerequisites
and implementing selected inward-owned interfaces. Reovim does not contain,
name, or depend on that OS implementation.

`no_std` means a closed source/dependency profile does not link Rust `std`. It
does not mean allocator-free full editing, no runtime prerequisites, every
target is supported, or every Reovim package is portable.

## Portable Core Profile

`kernel/foundation` is `#![no_std]` and uses Rust `core` only. It owns scalar
identity, version, bounds, carrier descriptors, and other allocation-free
language primitives.

The profile contains no `alloc`, `std`, environment, target, adapter, Domain,
Client, transport, presentation, extension, or Assembly dependency.

## Portable Allocation Profile

The full portable editing closure is `#![no_std]` and may use `core + alloc`.
It contains the selected:

- Kernel Interface, Kernel Core, and Kernel Runtime packages;
- public Rust SDK contracts needed by the composition;
- Resource-native models and pure Domain/Representation policies;
- common Client contracts; and
- Rust Family Core/Runtime/Module policy needed by at least one Client path.

Portable packages have no default `std` feature and no concrete environment,
transport, device, filesystem, process, or presentation dependency. Owned
variable-size contract values use `alloc` carriers. Full editing is not claimed
for a host without an allocator.

A package declares the atomic widths, lock-free assumptions, panic/unwind
behavior, and other target properties required by each mechanism. Unsupported
target properties fail composition or capability negotiation explicitly.

## Hosted Profile

Standard Host and Resource Adapters, production Protocol and Presentation
Adapters, process/extension supervisors, and current v0.16 application Assembly
roots may use `std` and admitted dependencies.

These packages remain outside portable closures and implement inward-owned
contracts. A hosted implementation cannot leak a `std`, platform, or concrete
adapter type back into a portable public surface.

## Portable Host Requirements

A full external custom-host composition supplies or selects:

1. Rust `core` and a compatible `alloc` implementation/global allocator;
2. declared atomic and memory-order behavior for selected mechanisms;
3. root entrypoint plus panic and termination policy;
4. separate Assembly-selected Kernel and Client ExecutionFacility
   realizations satisfying bounded admission, progress, cancellation, fencing,
   supervision, and shutdown;
5. exact Host Capability implementations consumed by Kernel, each Client, and
   each Resource path;
6. Resource Adapters for selected subjects;
7. Protocol Adapters only for selected remote or service paths;
8. Presentation Adapters for selected device/input/output paths; and
9. Assembly constructing independent Kernel and Client Runtime subtrees.

Items 1–4 are runtime and composition prerequisites. They do not create raw
allocator, mutex, thread, interrupt, panic, device, or syscall Host
Capabilities. Items 5–8 remain distinct roles even when one custom OS supplies
all of them.

## Host contracts across Server and Clients

The same Host Capability contract family applies to Kernel Runtime, every
Client Runtime/Family, and Resource Adapters. Each consumer receives only the
capabilities its accepted public journey uses.

Each grant and adapter instance has an explicit owner, incarnation, bounds,
readiness, failure accounting, replacement, and shutdown. An underlying host
service may be safely shared behind these interfaces, but it never creates one
global Host handle or merges Kernel and Client lifecycle authority.

Terminal, browser, window, GPU, and device input/output stay behind Client
Presentation Ports. They do not become Host Capabilities merely because the
host environment provides them.

ExecutionFacility is an Assembly-selected Runtime-owned private mechanism, not
a Host Capability or public executor. A custom host may provide its realization
without granting scheduler authority to SDKs, modules, Clients, or adapters.

## Capability growth

The initial Host API remains MonotonicClock and DiagnosticOutput. `no_std` or a
custom host does not pre-admit storage, signals, randomness, threads, allocators,
or another service.

A new Host Capability still requires one real consumer, typed semantics and
limits, standard and deterministic adapters where the claimed profile applies,
fault/lifecycle/Event evidence, and measurements. A custom-host-only adapter
additionally declares its exact target and support evidence.

## Conformance and claims

Portable profile evidence includes:

- a separately compiled `#![no_std]` public consumer;
- exact feature and Product dependency closure;
- static absence of `std` and concrete implementation types/imports;
- one runnable deterministic portable Client–Server composition through
  injected interfaces;
- allocator exhaustion, target-property, pressure, cancellation, panic
  boundary, readiness, rollback, and shutdown evidence as applicable; and
- exact code-size, memory, allocation, reaction-latency, and instrumentation
  measurements.

Hosted-target compilation of a `no_std` library proves only the closed portable
source/dependency profile. It does not qualify an unnamed OS. A specific custom
host claim requires an exact Supported and Qualified Environment Matrix row,
target/toolchain, adapters, Assembly, artifacts, runtime scenarios, faults,
measurements, and limitations.
