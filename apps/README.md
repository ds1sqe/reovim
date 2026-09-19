# apps

The apps: what actually runs, built on the Clients, Plugins and
Modules below.

- `reovim`: the binary. `server` boots the Kernel with every linked
  Module; `cli` and `tui` drive a server from the shell and the
  terminal; `view` reads a display's own directory and draws it,
  without holding it; bare `reovim FILE...` runs both in one process,
  the first FILE shown and the rest opened as drafts.
- `web`: the browser app. An editor page opens the gui Client over a
  WebSocket, bridged by `bridge.mjs` to reovim's Unix socket.
