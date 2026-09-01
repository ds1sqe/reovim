# Extension Contracts

## Purpose and v0.16 scope

Reovim treats extensibility and author experience as product boundaries. The
public SDK is not a convenience wrapper around private Core types. Kernel,
Client, Family, Module, and Client–Client Model (CCM) contracts plus the
independently versioned language-neutral schemas are semantic authority.

The extension system provides:

1. a trusted, in-process Rust SDK implemented and admitted first;
2. one language-, transport-, and serialization-neutral service contract;
3. out-of-process Python and TypeScript SDK projections admitted after the
   predecessor gates defined below; and
4. a language-neutral Extension Adapter contract, optional Extension Engine
   Adapter specialization, Adapter Kit, `TestOnly` deterministic model, and
   Quick Extension Profile.

No extension receives a private Buffer, History, Projection, Runtime,
ExecutionFacility, Host Adapter, Resource Adapter, View, device, or transport
handle. First-party modules use the same public paths available to a third
party with the same role and grants. If required behavior needs a private
shortcut, the public contract must be improved, the behavior rebuilt above it,
or the behavior removed from the release.

v0.16 selects no production interpreter, VM, managed engine, embedded Python,
Lua, Haskell runtime, WebAssembly engine, native ABI, sandbox, engine topology,
or Neovim compatibility profile. It also selects no separate
configuration/automation role. Naming a candidate language or engine creates
no implementation, support, or qualification claim.


## Chapter map

All chapters below are normative parts of this Extension contract. This split
changes navigation only.

1. [Packages, Contribution Points, and registry](01-Packages-Contribution-Points-and-Registry.md)
2. [SDKs, service contracts, and compatibility](02-SDKs-Service-Contracts-and-Compatibility.md)
3. [Adapters, execution, and lifecycle](03-Adapters-Execution-and-Lifecycle.md)
4. [Quick authoring, admission, and compatibility](04-Quick-Authoring-Admission-and-Compatibility.md)
