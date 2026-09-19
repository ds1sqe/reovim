# clients

The client side: the Clients themselves and the libraries they link.

- `core`: the library every Client and Plugin host links: the `Tree`
  over a carrier, and the `Host` that hands a Plugin its `Calls` over
  that tree and the Client's own under `/self` — its devices as cells,
  its Buffers, and its Drivers under `/self/bin`.
- `core/api`: what the Client's own Driver and the Plugins over it
  both spell: the keyboard presses, a region's size and the View it
  answers, and the `open` a Plugin asks its Client for.
- `core/region`: what only a Plugin uses over one region of
  `/self/dev`: the cells it reads and writes, the `view` cell it
  serves, and the Buffer every Plugin that shows text opens its own
  way.
- `sh`: the shell: a typed line in, text out, over a `Calls`; the
  line and its editing, PATH, and the decoder from a line to the
  record a Driver under `/bin` declares.
- `tui`: the terminal Client: one region of a terminal laid out from
  `/self/dev`, knowing no Plugin of its own.
- `gui`: the browser Client, in TypeScript, used by `apps/web`.
- `rig`: the test rig: a Kernel and Clients wired up in one
  process, for tests only.
