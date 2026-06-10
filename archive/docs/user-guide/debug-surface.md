# Debug Surface (CLI)

Open a client-debug driver's wire surface from the command line: probe
metadata, observe streams of frames, and send one-shot drive commands.

## Overview

The debug surface is a **driver-owned** observability channel. A driver
cdylib implements the `ClientDebugSurface` trait; the CLI is a dumb
transport over the `ClientDebugService::DebugStream` bidirectional RPC.
Bytes flow opaque end-to-end — only driver names and schema names are
structured at the flag layer.

```
reovim-cli ──► reovim-server ──► ClientDebugRegistry ──► loaded cdylib
    probe                 DebugStream             LoadedClientDebug
    observe      bidirectional bytes              probe / observe / drive
    drive
```

This design means:

- **Driver authors own the wire vocabulary.** The CLI doesn't know what
  `poc-frames` frames mean, or what `poc-echo` accepts — the driver does.
- **No schema knowledge in the transport.** The proto carries `bytes`;
  encoding choices (JSON, CBOR, flat buffers, raw binary) are the
  driver's problem.
- **One vtable per cdylib, optional per driver.** Implement the trait,
  call `declare_client_debug_driver!`, and the host can scan and load
  the cdylib.

## Quick Start

The Phase 0 PoC cdylib ([`tools/driver-debug-poc/`](../../tools/driver-debug-poc/src/lib.rs))
ships with the workspace as a test fixture. The fastest way to see the
three verbs in action is to run the E2E suite with `--nocapture`:

```bash
cargo build -p reovim-driver-debug-poc
cargo build -p reovim-app-cli
cargo test -p reovim-client-cli --test debug_verbs_e2e -- --nocapture
```

That harness spawns a real `reovim-server` on an OS-assigned port,
loads the PoC cdylib into a `ClientDebugRegistry`, and drives it with
the real `reovim-cli` binary. Inspect
[`clients/cli/tests/debug_verbs_e2e.rs`](../../clients/cli/tests/debug_verbs_e2e.rs)
and its
[`common.rs`](../../clients/cli/tests/common.rs) harness to see the
composition-root wiring.

A standalone composition root (one that hosts arbitrary drivers without
going through the test harness) is not shipped in-tree yet — Phase 1
installs an empty-registry default. See
[Debug Surface Architecture §7 Extension Recipe](../architecture/debug-surface.md)
for how to wire one.

## Verb Reference

All three verbs open a single `DebugStream`, send the handshake, then
either print one response and exit (`probe`, `drive`) or stream frames
(`observe`).

### `reovim cli debug probe`

Fetch a driver's `DebugProbe` metadata: name, description, and the
lists of observe- and drive-schema names the driver advertises.

```
reovim cli [--grpc ADDR] debug probe --driver NAME [--format plain|json]
```

| Flag | Default | Meaning |
|---|---|---|
| `--driver NAME` | required | Driver name as registered in the server. |
| `--format plain\|json` | `plain` | Output format (see [Output Formats](#output-formats)). |

Example against the PoC:

```
$ reovim cli --grpc 127.0.0.1:54321 debug probe --driver debug-poc
driver: debug-poc
description: Phase 0 debug-surface PoC
observe_schemas: poc-frames
drive_schemas: poc-echo
```

### `reovim cli debug observe`

Start an observer on the given schema and print each frame until the
server closes the stream or `--count N` frames have been received.

```
reovim cli [--grpc ADDR] debug observe --driver NAME --schema SCHEMA
    [--count N] [--format plain|json]
```

| Flag | Default | Meaning |
|---|---|---|
| `--driver NAME` | required | Driver name as registered in the server. |
| `--schema SCHEMA` | required | One of the driver's `observe_schemas`. |
| `--count N` | unset | Stop after N frames (sends `ObserveStop`). Without `--count`, the CLI half-closes the outbound stream and waits for natural EOS. |
| `--format plain\|json` | `plain` | Output format. |

Example against the PoC (three frames, then EOS):

```
$ reovim cli --grpc 127.0.0.1:54321 debug observe \
    --driver debug-poc --schema poc-frames
frame[0]=6672616d652d32
frame[1]=6672616d652d31
frame[2]=6672616d652d30
```

The hex values decode to `frame-2`, `frame-1`, `frame-0` — the PoC's
countdown frame body. The CLI never decodes driver payloads; all
bodies are reported as opaque bytes.

### `reovim cli debug drive`

Send a one-shot drive command on the given schema and print the single
response. `--input` accepts either an inline string or `@path` to read
raw bytes from a file.

```
reovim cli [--grpc ADDR] debug drive --driver NAME --schema SCHEMA
    --input SPEC [--format plain|json]
```

| Flag | Default | Meaning |
|---|---|---|
| `--driver NAME` | required | Driver name as registered in the server. |
| `--schema SCHEMA` | required | One of the driver's `drive_schemas`. |
| `--input SPEC` | required | Inline UTF-8 string, or `@/abs/path` to read the file as raw bytes. |
| `--format plain\|json` | `plain` | Output format. |

Example against the PoC (the PoC's `poc-echo` schema echoes the
input):

```
$ reovim cli --grpc 127.0.0.1:54321 debug drive \
    --driver debug-poc --schema poc-echo --input hello
response=68656c6c6f
```

File input:

```
$ printf '\x01\x02\x03\x04' > /tmp/driver-cmd.bin
$ reovim cli --grpc 127.0.0.1:54321 debug drive \
    --driver debug-poc --schema poc-echo --input @/tmp/driver-cmd.bin
response=01020304
```

## Output Formats

### `plain` (default)

Human-readable key/value lines. Observe frames are `frame[I]=HEX`,
one per line; drive responses are `response=HEX`. Non-ASCII or binary
payloads are hex-encoded so the output stays copy-paste safe.

### `json`

Machine-readable. Identical information, but frame and drive bodies
are base64-encoded (not hex) and wrapped in a single JSON object per
verb:

```
$ reovim cli ... debug probe --driver debug-poc --format json
{"driver":"debug-poc","description":"Phase 0 debug-surface PoC","observe_schemas":["poc-frames"],"drive_schemas":["poc-echo"]}

$ reovim cli ... debug observe --driver debug-poc --schema poc-frames --format json
{"frames":["ZnJhbWUtMg==","ZnJhbWUtMQ==","ZnJhbWUtMA=="]}

$ reovim cli ... debug drive --driver debug-poc --schema poc-echo --input hi --format json
{"response":"aGk="}
```

The `json` output is always a single line (newline-terminated by the
shell), suitable for piping into `jq` or capturing into a variable.

## Troubleshooting

| Error | Meaning | Fix |
|---|---|---|
| `unknown driver: NAME` | The server's registry has no entry under this name. | Check the composition root's `registry.register(NAME, ...)` call matches. |
| `observe: unknown schema NAME` / `drive: unknown schema NAME` | The driver does not advertise this schema in its `DebugProbe`. | Run `probe` first to list the driver's real schema names. |
| `stream closed before probe response` | Server hung up before sending a `ProbeResp`. | Check server logs — the driver's `probe()` vtable slot may have panicked. |
| `failed to read PATH: ...` | `--input @PATH` could not open the file. | Use an absolute path; check permissions. |
| Connection refused / timeout | Server not running or wrong `--grpc` address. | Start the server and confirm the gRPC port with `ss -tlnp` or your server's stderr. |
| CLI never returns during `observe` | The PoC emits its frames synchronously; for a live driver that blocks on an external event, the stream stays open until the driver EOSes. | Use `--count N` to cap reception, or Ctrl-C the CLI. |

If you see `Status::internal` surfaced as an opaque RPC error, the
server has evicted the driver due to an FFI panic (`rc == -2` on a
vtable trampoline). The driver entry is gone from the registry;
re-registering from the composition root is required to recover. See
[Architecture §4 Registry + State Machine](../architecture/debug-surface.md)
for the eviction path.

## Limitations (current)

- **No `probe` without `--driver`.** Cross-driver discovery (e.g.
  `reovim cli debug probe --all`) requires the `pkg`-based manifest
  index scheduled for epic #771. Today you must already know the
  driver's registered name.
- **No in-tree composition root that hosts real drivers.** Phase 1
  shipped an empty-registry default; the E2E test harness is the only
  path that populates a registry today. Real-driver registration
  arrives with #769 Phase 5 (TUI render driver cdylib migration) and
  the subsequent #770 Phase 3 follow-up.
- **Only one driver instance per cdylib.** A cdylib exports exactly
  one `REOVIM_CLIENT_DEBUG_DRIVER_VTABLE`; hosting N instances requires
  N cdylib files.
- **Observer is single-subscriber.** The trait returns `&mut self` on
  `observe()`, so at most one observer can be live on a driver
  instance at a time. A second `ObserveStart` (via a second CLI
  invocation, or a second `Select` in the same stream) waits behind
  the registry mutex.

## See Also

- [Debug Surface Architecture](../architecture/debug-surface.md) —
  layer map, ABI, registry, `DebugStream` proto, extension recipe.
- [Driver ABI v1](../architecture/driver-abi-v1.md) — the portable
  contract the cdylib implements.
- [CLI Reference](./cli-reference.md) — general `reovim cli` usage,
  `--grpc`, output formatting.
