# plugins

Plugins, each hosted at a Client.

- `cli`: the cli buffer: a typed line in a region of its own, run by
  `clients/sh` as the process cli runs it.
- `explorer`: a Container listed in a Buffer, with the keys that
  walk it.
- `markdown`: the looks a `.md` draft's constructs are, served over
  the Buffer to whoever draws it; it holds no region.
- `vim`: keys in, calls out, over the text and the cursor it holds.
