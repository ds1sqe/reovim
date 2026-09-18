# clients

The client side: the Clients themselves and the libraries they link.

- `core`: the library every Client and Plugin host links: the
  Client, the Tree, capabilities, input.
- `cli`: the command-line Client: a line in, text out, over the
  Tree; knows no Module and no Plugin.
- `tui`: the terminal Client: one region of a terminal, with a
  Plugin drawing an rd in it.
- `gui`: the browser Client, in TypeScript, used by `apps/web`.
- `rig`: the test rig: a Kernel and Clients wired up in one
  process, for tests only.
