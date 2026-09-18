# host

The host: the one place that touches the real machine.

- `os`: the only crate that knows the host. It picks a backend at
  build time — hosted, using the OS, or empty, answering
  `Unsupported` — so everything above it stays `no_std` + alloc.
