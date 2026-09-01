# Reovim

[![Crates.io](https://img.shields.io/crates/v/reovim.svg)](https://crates.io/crates/reovim)

Reovim is a high-performance, extensible editing system pursuing mission-
critical quality. It is designed to inspect and edit addressable Resources
through faithful low-level and Domain-aware high-level Representations. Text is
one Domain, not the shape of the core.

## Status

v0.16 is the editing-system re-foundation. The normative target contracts are
ratified in [Documentation](Documentation/README.md); their presence does not
claim that the current source implements them or that an artifact is supported
or qualified. The former OS-shaped implementation has been removed from the
active workspace. The remaining hosted workspace is a tooling-only baseline;
fresh implementation and selective migration proceed through reviewed,
contract-owned slices.

v0.17 follows with Neovim feature parity built only on the v0.16 public
contracts.

## Architecture

The portable **Reovim Kernel** owns Domain-neutral editing contracts and generic
mechanisms. It is not an operating-system kernel. A Resource Adapter connects
data; a Domain supplies meaning; a Representation exposes interpreted state; a
Projection synchronizes it; and a Client-owned View presents it.

Clients own their applicable presentation or output policy. Any View, viewport,
layout, input mapping, drawing, rendering, or frame state that exists is
Client-owned. TUI, CLI, and Web are mandatory v0.16 Client Families, each with a
Core, public Contract, Module surface, Runtime, and production Presentation or
output path.

Start here:

- [Normative map and invariants](Documentation/README.md)
- [Concept](Documentation/00-Concept.md)
- [Vocabulary](Documentation/00-Vocabulary.md)
- [Architecture map](Documentation/01-Architecture/README.md)
- [Execution and concurrency](Documentation/01-Architecture/04-Execution/01-Execution-Laws-and-Operation-Classes.md)
- [Client, CLM, CCM, and rendering](Documentation/01-Architecture/05-Client/README.md)
- [Extension contracts](Documentation/03-Extensions/README.md)
- [Performance contract](Documentation/04-Performance/README.md)
- [Rule matrix](Documentation/06-Conformance/02-Rule-Matrix.md)
- [Heritage](Documentation/heritage/README.md) — non-normative historical
  material

## Quality and performance

Source fidelity, atomic state authority, explicit Effect Certainty, bounded
behavior, recovery, and debuggability take priority over feature count.
Input-to-present p99 is the primary reaction metric. Numeric budgets follow
reproducible baselines and retained real-scenario data rather than unsupported
targets.

Files under `perf/` and the changelog are historical donor evidence. They do
not establish v0.16 support, conformance, qualification, or current performance.

Run the repository check suite with:

```sh
./scripts/check.sh
```

## License

AGPL-3.0. See [LICENSE](LICENSE). For commercial licensing, contact
ds1sqe@mensakorea.org.
