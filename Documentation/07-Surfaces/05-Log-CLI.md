# 7.5 — Log CLI

**Scope.** The user-facing log reader: `reovim log` — the `dmesg`
analog over the 9.5 logging substrate (ring, stream, sink).

**Heritage.** `dmesg` / `journalctl` conventions; the v0.15
`cli log-tail` command (archived).

**Locked rules.** `LOG9..LOG11` (mechanism rules `LOG1..LOG8` live
in 9.5).

---

## 1. Command set

```
reovim log                       # dump the ring (LOG6); plain LOG2 lines
reovim log -w | --follow         # follow mode: stream new events live
reovim log -T | --ctime          # wall-clock timestamps instead of [seconds.micros]
reovim log -l | --level <l>[,<l>...]   # only the listed levels (trace..error)
reovim log -H | --human          # pager + level colors + relative times
reovim log -C | --clear          # clear the ring (requires debug.mutate)
```

`dmesg` mapping, member for member:

| `dmesg` | `reovim log` | Notes |
|---|---|---|
| `dmesg` | `reovim log` | ring dump, oldest first |
| `dmesg -w` | `reovim log -w` | tail -f semantics; ring dump first, then live |
| `dmesg -T` | `reovim log -T` | human timestamps (§4) |
| `dmesg -l err,crit` | `reovim log -l warn,error` | level list (§5) |
| `dmesg -H` | `reovim log -H` | pager, colors, relative times |
| `dmesg \| grep -i pcie` | `reovim log \| grep -i scheme` | composes; see LOG10 |
| `dmesg -C` (root) | `reovim log -C` (`debug.mutate`) | clear before reproducing (§6) |

Flags combine where meaningful (`-w -l error`, `-T -l warn,error`).
`-C` combines with nothing.

## 2. Sources

> **LOG9 — The reader is a pure debug-surface client.** `reovim log`
> reaches the kernel exclusively through the 7.2 surface:
>
> - dump: `hostapi_debug_log_ring_read` (`debug.read`),
> - follow: `hostapi_debug_event_subscribe` with the requested
>   filter, rendering each event per LOG2 client-side,
> - clear: `hostapi_debug_drive_log_ring_clear` (`debug.mutate`).
>
> It has no privileged channel and no live file access — the LOG1
> single-pipeline rule applies to readers as much as writers.
> *Class*: spec-asserted + conformance.

Postmortem (server not running) needs no reader: the LOG7 file sink
is already plain LOG2 text — `grep` it directly.

## 3. Output contract

> **LOG10 — Plain, composable output by default.** Without `-H`,
> output is exactly the LOG2 canonical lines: UTF-8, one event per
> line, no color, no escapes, no pager — byte-stable for scripts and
> pipes regardless of whether stdout is a TTY. `-H` is the only
> opt-in to decoration: pager, per-level colors, relative times.
> Decorated output is presentation, never a parsing target.
> *Class*: conformance.

There is no built-in pattern filter; `grep` composes (do one thing
well). There is no JSON mode; structured consumers subscribe to
DS12 directly (7.2, 9.5 open item 3).

## 4. Timestamps

`hostapi_debug_log_ring_read` returns the entries plus the boot
wall-clock anchor (`CLOCK_REALTIME` captured by `Init::new`, boot stage 0).
`-T` renders `anchor + monotonic_ts` as local wall-clock time.

Caveat (same as `dmesg -T`): the anchor does not track wall-clock
adjustments after boot (NTP steps, suspend); `-T` times are
approximate. The default monotonic form is the truth.

## 5. Level filter

`-l` takes a comma-separated subset of `trace, debug, info, warn,
error`. Ring entries carry their `LogLevel` alongside the rendered
line (LOG6), so dump-mode filtering needs no line parsing; follow
mode pushes the filter into `EventFilter` server-side (9.4 §7) so
unwanted events never cross the transport.

## 6. Clear

> **LOG11 — Ring clear is a drive operation.** `reovim log -C`
> invokes `hostapi_debug_drive_log_ring_clear`, which requires the
> `debug.mutate` capability (DS13) and emits the standard
> `debug.drive.start|ok` audit events (`op=log_ring_clear`,
> correlation-ID-tagged). The clear drops ring entries only — the
> boot anchor survives, the file sink is untouched, and the audit
> event of the clear is itself the first entry of the emptied ring,
> so a cleared ring is self-documenting.
> *Class*: runtime.

The intended workflow is `dmesg`'s: clear, reproduce the bug, dump a
ring that contains only the reproduction.

## 7. Capability summary

| Operation | Needs |
|---|---|
| dump, `-w`, `-T`, `-l`, `-H` | transport auth + `debug.read` |
| `-C` | transport auth + `debug.mutate` |

Both capabilities are the coarse DS5 pair declared at attach
(7.2 §5.1); the reader declares only what its flags require.

## Open items

1. Time-window flags (`--since` / `--until`). Draft: deferred;
   entries carry timestamps, `awk` composes.
2. Whether `-w` should reconnect on server restart. Draft: no —
   exit with a named error; supervision is the caller's job.

## Conformance

| Rule | Fixture |
|---|---|
| LOG9 | Reader binary traffic audit: only the three named surface ops appear; no file opens in live mode. |
| LOG10 | Piped dump is byte-identical with and without a TTY; `-H` output never appears without the flag. |
| LOG11 | `-C` without `debug.mutate` → `PermissionDenied`; with it, ring empties, audit event present as first new entry, sink file unaffected. |
| §4 | `-T` rendering equals anchor + monotonic for a fixture anchor. |
| §5 | `-l warn,error` dump excludes info lines without parsing rendered text (level metadata used); follow-mode filter applied server-side (transport capture shows no filtered events). |
