# Reovim

Reovim is a high-performance, extensible editing system. Text is one domain,
not the shape of its core. Correctness, debuggability and measured performance
take priority over feature count.

## Current direction

v0.16 rebuilds the editing-system foundation. The first product target is a
Markdown editor with a TypeScript GUI and a TUI with Kitty graphics. GUI
framework selection remains open; SSR is the initial direction.

The local Rust service executes open, semantic edits, branching undo/redo and
automatic snapshots through Core/Runtime for two small native models. It is
not a usable editor yet: full Markdown and client presentation remain
unfinished. Migration extracts useful behavior into domain-neutral mechanisms.
v0.17 follows with Neovim feature parity.

## Start here

- [Product goals](Documentation/README.md)
- [Core design](Documentation/Design/00-core.md)
- [Design chapters](Documentation/Design/README.md)
- [Debugging](Documentation/debugging.md) and
  [performance](Documentation/performance.md)
- [Contributing](CONTRIBUTING.md) and [coding style](CODE_STYLE.md)

## Check the current code

Run the [local editing reference](apps/server/README.md):

```sh
cargo run --manifest-path product/Cargo.toml -p reovim-server --features std
```

It boots two public consumers, edits heading/keyed state, traverses retained
branches, observes results, closes owners and exports diagnostics to stderr.
Contributor checks:

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
