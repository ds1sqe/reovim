# apps

The apps: what actually runs, built on the Clients, Plugins and
Modules below.

- `reovim`: the binary. `server` boots the Kernel with every linked
  Module; `cli` and `tui` drive a server from the shell and the
  terminal; bare `reovim FILE` runs both in one process.
- `web`: the browser app. An editor page opens the gui Client over a
  WebSocket, bridged by `bridge.mjs` to reovim's Unix socket.
