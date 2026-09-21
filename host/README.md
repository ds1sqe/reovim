# host

The host: the one place that touches the real machine.

- `os`: the only crate that knows the host. It picks a backend at
  build time — hosted, using the OS, or empty, answering
  `Unsupported` — so everything above it stays `no_std` + alloc.

`os::dirs` selects runtime, working and config directories; selecting the
config directory creates nothing. `os::fs::read_all` reads one opened file
to EOF, retaining that file across atomic pathname replacement. Concurrent
in-place writers do not provide an immutable filesystem snapshot.
