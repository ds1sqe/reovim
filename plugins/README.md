# plugins

Plugins, each hosted at a Client.

- `cli`: the cli buffer: the cli's line in a region of its own,
  run by `clients/cli` as the process cli runs it.
- `explorer`: a Container listed in a Buffer, with the keys that
  walk it.
- `vim`: keys in, calls out, over the text and the cursor it holds.
