# Reovim

Reovim is a high-performance, extensible editing system. Text is one domain,
not the shape of its core. Correctness, debuggability and measured performance
take priority over feature count.

## Current direction

v0.16 rebuilds the editing-system foundation. The first product target is a
Markdown editor with a TypeScript GUI and a TUI with Kitty graphics. GUI
framework selection remains open; SSR is the initial direction.

The source currently contains portable Foundation/Interface contracts and
public-consumer fixtures, not a running editor. Migration extracts useful
behavior from the stable editor into new domain-neutral mechanisms.
v0.17 follows with Neovim feature parity.

## Start here

- [Goals and interface map](Documentation/README.md)
- [Kernel mechanisms, source tree and Host boundary][architecture]
- [Editing and file operations](Documentation/interfaces/resources-and-state.md)
- [Events, keymaps and resolver](Documentation/interfaces/events-and-input.md)
- [Clients and rendering](Documentation/interfaces/clients.md)
- [Modules, SDKs and boot lifetime][modules]
- [Debugging](Documentation/debugging.md) and
  [performance](Documentation/performance.md)
- [Contributing](CONTRIBUTING.md) and [coding style](CODE_STYLE.md)

## Check the current code

```sh
./scripts/check.sh
./scripts/ci.sh
```

The first command reuses successful toolchain-bound results and selects changed
components plus dependents. The second runs the same full matrix as GitHub CI,
using the exact stable/nightly versions in [ci.json](ci.json). It installs the
selected toolchains/targets and fetches dependencies; network access is needed.
Normal runs do not change the window. See CONTRIBUTING.md for updates.
Product Cargo commands use --manifest-path product/Cargo.toml.

The specification is a target, not evidence that the editor is implemented.
Release scope and SDK obligations are stated once in Documentation.
Historical references remain in [heritage](Documentation/heritage/README.md)
and [performance reports](perf/README.md), as well as Git/reference worktrees.

## License

AGPL-3.0. See [LICENSE](LICENSE). For commercial licensing, contact
ds1sqe@mensakorea.org.

[architecture]: Documentation/architecture.md
[modules]: Documentation/interfaces/lifecycle-and-modules.md
