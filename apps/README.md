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

The terminal app captures `reovim/init` in the config directory before
starting a Plugin. Its lines run once at the first Vim frame through the
same command path as typed lines. Blank lines and leading `#` comments are
skipped. Diagnostics go to `dmesg`; an error does not stop the next line,
while a successful region switch or quit ends the script. Typed commands'
partial effects remain after an error; this is not transactional settings
validation. Config selection belongs to the host adapter.
