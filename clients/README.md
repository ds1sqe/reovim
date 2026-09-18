# clients

The client side: the Clients themselves and the libraries they link.

- `core`: the library every Client and Plugin host links: the
  Client, the Tree, capabilities, input.
- `sh`: the shell: a typed line in, text out, over the Tree; the
  line and its editing, PATH, and the decoder from a line to the
  record a Driver under `/bin` declares.
- `tui`: the terminal Client: one region of a terminal, with a
  Plugin drawing an rd in it.
- `gui`: the browser Client, in TypeScript, used by `apps/web`.
- `rig`: the test rig: a Kernel and Clients wired up in one
  process, for tests only.
