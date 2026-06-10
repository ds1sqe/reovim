# Web Client

Browser-based editor using gRPC-Web and WASM.

## Source Location

`clients/web/`

## Architecture

- **TypeScript + WASM**: Core editor logic compiled to WebAssembly
- **gRPC-Web**: Communication with the server via gRPC-Web protocol
- **Rendering**: Browser-based rendering with syntax highlighting and theme support

## Features

- Browser-based editing with full syntax highlighting
- gRPC-Web transport for server communication
- Theme support
- Screen capture support
- Playwright-based test suite

## Related Documents

- [Client Overview](./overview.md) - Client architecture
- [TUI Client](./tui.md) - Terminal interface
- [CLI Client](./cli.md) - Command-line interface
