# Client, CLM, CCM, and Rendering

> **Normative v0.16 target contract.**
>
> This chapter defines Client composition, Client-family extension surfaces,
> cross-Client synchronization, and Rendering Capability ownership. It sets the
> mandatory v0.16 Client-family release scope. Its present-tense language is
> not implementation, support, package, or qualification evidence.

## 1. Scope and authority

This chapter owns:

- the Client Layer Model (CLM) inside one Client Runtime;
- the Client–Client Model (CCM) for explicit cross-Client synchronization;
- the common Client Semantic Contract;
- each Client Family's Core, Contract, and Module surfaces;
- Client-local View, layout, render, frame, and presentation ownership;
- Rendering Capability negotiation and loss/recovery behavior;
- Client and module synchronization lifecycle; and
- the mandatory TUI, CLI, and Web family release boundary.

The Reovim Kernel owns editing state and semantic interaction mechanisms. It
does not own a concrete viewport, layout, input mapping, drawing model, frame,
terminal, browser, display, or presentation target. A Client receives stamped
semantic state and submits semantic Intents through public contracts.

The SDK and Extension contracts own language binding, binding scope, delivery
gates, and admission status. A mandatory Client Family does not implicitly
select a language binding, extension package, process topology, transport,
renderer, or optional backend.


## Chapter map

All chapters below are normative parts of this Client contract. This split changes
navigation only.

1. [Client ownership and Family contracts](01-Client-Ownership-and-Family-Contracts.md)
2. [Rendering Capabilities](02-Rendering-Capabilities.md)
3. [Extension Modules and CCM](03-Extension-Modules-and-CCM.md)
4. [Lifecycle, Client Families, and release evidence](04-Lifecycle-Families-and-Release-Evidence.md)
