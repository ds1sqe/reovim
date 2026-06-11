# 9.5 — Logging

**Scope.** The human-readable rendering of the DS12 stream: the
canonical line format, the emitter address scheme, the kernel log
ring, the file sink, early-boot output, verbosity flags, and the
cdylib emission entry point.

**Heritage.** Linux `printk` / `dmesg` conventions; the v0.15 printk
subsystem (archived).

**Locked rules.** `LOG1..LOG8` (reader rules `LOG9..LOG11` in 7.5;
launcher flags `AL11` in 1.3).

---

## 1. One mechanism

> **LOG1 — Logging is a rendering of DS12; there is no second
> pipeline.** Every log line is the deterministic rendering of
> exactly one DS12 event (9.4). Nothing reaches a log that did not
> pass through the DS12 stream — same subscriber model, same rate
> limits and coalescing, same redaction classes. A logger is a DS12
> subscriber with a formatter; it has no privileged channel.
> *Class*: spec-asserted + conformance.

This is the mechanism/policy split applied to diagnostics: DS12 is
the mechanism; "a log file", "stderr output", "log-tail over the
debug surface" are all policies layered on the one stream.

## 2. Canonical line format

> **LOG2 — Canonical line grammar.**
>
> ```
> line := "[" ts "]" SP emitter SP address [SP instance] ":" SP message
> ts   := seconds "." micros   ; monotonic since kernel boot,
>                              ; seconds right-aligned min width 5,
>                              ; micros zero-padded width 6
> ```
>
> One event per line. UTF-8, no ANSI escapes. Embedded newlines in
> `message` are escaped as `\n`. The rendering of a given event is
> byte-deterministic.
> *Class*: runtime + CI golden tests.

Annotated example — a cdylib module event:

```
[  123.456789] vim-mode module/7.0 main/b3.w1: entered Insert at byte 4096
   │           │        │          │
   │           │        │          └─ instance address: session "main",
   │           │        │             buffer 3, window 1 (§4)
   │           │        └─ emitter address: kind/CdylibId.vtable (§3)
   │           └─ package name from the emitting cdylib's manifest
   └─ seconds since kernel boot (monotonic clock), microsecond precision
```

A kernel-emitted event:

```
[    0.002413] kernel init: boot.stage.ok stage=2 name=config
   │           │      │     │
   │           │      │     └─ rendered structured event (family.subject
   │           │      │        plus key fields; §5)
   │           │      └─ kernel subsystem in the address position (§3)
   │           └─ the kernel itself is the emitter
   └─ early-boot lines exist from stage 0 (§8)
```

Field sources: `ts` from the event timestamp re-based to boot;
`emitter` and `address` are host-derived (§3, never caller-supplied);
`instance` from the DS12 common scoping fields (9.4 §5); `message`
per §5.

## 3. Emitter address

The address answers *where in the loaded topology* the event came
from — the analog of a PCI address in `dmesg`. For cdylib emitters:

```
module/7.2
  │    │ │
  │    │ └─ vtable index — one cdylib can export multiple vtables
  │    │    (8.3); 0-based in manifest declaration order
  │    └─── CdylibId — interned at load (NonZeroU32), stable for the
  │         life of the process; reproducible across identical boots
  │         because load order follows the lockfile (DT17)
  └──────── kind — fixed string per ManifestKind (table below)
```

| `ManifestKind` (6.3 §3) | address kind |
|---|---|
| `ModuleServer` | `module` |
| `DriverServer` | `driver` |
| `DomainServer` | `domain` |
| `ProviderServer` | `provider` |
| `StreamScheme` | `scheme` |
| `ModuleClient` | `cl-module` |
| `DriverClient` | `cl-driver` |
| `CapabilityClient` | `cl-cap` |

Kernel emitters put a bare subsystem name in the address position
(`init`, `dispatch`, `config`, `pkg`, `persist`, `stream`, …). A
kernel address never contains `/`; a cdylib address always does —
the two are unambiguous.

> **LOG3 — Host-derived emitter identity.** The `emitter` and
> `address` fields are stamped by the kernel from the dispatch
> context of the emitting call (the RefGuard / turn context, 2.3) or
> from the kernel subsystem that emitted. A cdylib cannot supply,
> spoof, or suppress its own identity. Like a PCI address, the
> CdylibId component is enumeration order — stable per process, not
> a persistent name; the persistent name is the package name in the
> `emitter` field.
> *Class*: kernel-enforced.

## 4. Instance address

The optional `instance` token scopes the event to runtime state,
derived from whichever DS12 common fields (9.4 §5) are present:

| Fields present | Rendered |
|---|---|
| session | `main` |
| session + buffer | `main/b3` |
| session + buffer + window | `main/b3.w1` |
| client only | `c2` |
| stream only | `s17` |
| none | *(token omitted)* |

The instance grammar is part of LOG2's golden rendering; the table
above is exhaustive for ABI major 1 and extends additively.

## 5. Message rendering

Two cases:

- **`log.message` events (free-form, §6):** `message` is the event's
  message field, verbatim (after `\n` escaping).
- **Structured families (OBS1):** `message` is `family.subject`
  followed by `key=value` pairs of the family's required fields, in
  schema order, values rendered per their JSON type. Fields whose
  redaction class (CFG7) forbids the sink's audience render as
  `key=<redacted>`.

A curated per-family template MAY replace the default `key=value`
rendering, but must remain deterministic and lossless for required
fields (open item 1).

## 6. Free-form diagnostics: `log.message`

> **LOG4 — `log.message` family.** Free-form diagnostics are DS12
> events of family `log.message` with required fields:
>
> - `level` — Trace / Debug / Info / Warn / Error,
> - `target` — dotted module-path-like string (e.g.
>   `vim_mode.motion.word`),
> - `message` — UTF-8, at most
>   `kernel.host.[limits].max-log-message-bytes` (default 4096;
>   longer messages are truncated, never split).
>
> The family is exempt from OBS3's closed per-family schema for the
> `message` field only; it is subject to OBS rate limits and
> coalescing like every family. Default redaction class:
> `internal`.
> *Class*: runtime.

## 7. Emission ABI

> **LOG5 — One emission entry point crosses the ABI.**
>
> ```c
> ErrorCode hostapi_log_emit(LogLevel level,
>                            StrSlice target,
>                            StrSlice message);
> ```
>
> Slices are borrowed for the duration of the call; the host copies.
> The host stamps timestamp, emitter, and address from the dispatch
> context (LOG3). Returns `ErrorCode::ResourceExhausted` when the
> caller's rate limit is exceeded (the event is coalesced, not
> dropped silently).
>
> Ext code emits free-form `log.message` only; structured OBS1
> families are kernel-owned. A structured emission entry point would
> be a future additive hostapi, not a change to this one.
> *Class*: ABI.

`LogLevel` is catalogued in 6.3 §2.4.

## 8. Log ring

> **LOG6 — Kernel log ring.** The kernel retains a bounded in-memory
> ring of rendered LOG2 lines — the `dmesg` analog. Each entry
> carries its `LogLevel` alongside the rendered line (so readers
> filter without parsing). Capacity:
> `kernel.host.[limits].log-ring-bytes` (default 1 MiB); eviction is
> oldest-first, whole entries. The ring is allocated before any
> cdylib loads and receives every event from boot stage 0 onward,
> regardless of subscriber or sink state. It is read through the
> debug read surface (`hostapi_debug_log_ring_read`, 7.2 §5;
> requires `debug.read`), which returns the entries plus the boot
> wall-clock anchor (7.5 §4). The user-facing reader is
> `reovim log` (7.5).
> *Class*: kernel-enforced.

## 9. File sink

> **LOG7 — File sink.** The host runtime ships one file-backed DS12
> subscriber writing LOG2 lines. Configuration, under
> `kernel.host.[log]`:
>
> | Key | Default | Meaning |
> |---|---|---|
> | `path` | `<state dir>/reovim.log` (2.4 state root) | sink target |
> | `level` | `info` | minimum level written |
> | `rotate-bytes` | 64 MiB | size-based rotation threshold |
> | `rotate-keep` | 4 | rotated files retained (`.1`..`.N`) |
>
> On open, the sink replays the ring head so the file contains the
> full boot (LOG8). Sink failures never affect editing: writes are
> dropped, a `log.sink.fail` event is emitted (visible via the ring
> and other subscribers), and the sink retries on rotation
> boundaries. This sink is the durability story for DS12 — the
> stream itself stays volatile (9.4 open item 2, resolved).
> *Class*: runtime.

### 9.1 Panic-time final flush

On a panic the `arch/` panic handler performs the **final flush**
(AB12, 6.2 §5 owns when and what): the panic renders as a LOG2
line into the one kernel log ring (§1 — there is only one log
mechanism and one buffer; a panic does not get a second log), and
the handler then synchronously flushes the ring tail to this
sink's target file via raw `arch/` syscalls — the normal LOG7
machinery is not trusted at panic time, but the file and the line
format are the same. This is the pstore analog: the ring is
volatile; the final flush is what survives the restart. Lifecycle
consequences (target state, quarantine) travel through the
persisted state (2.4), never by parsing log lines. At the `arch/`
seam, the registered ring-tail provider returns one contiguous
`&[u8]` already rendered in LOG2 line format; `arch/` writes it
verbatim ahead of the panic line and never parses ring entries. The flush path
lands with the `arch/` panic handler (the arch-foundation phase),
gated by the AB12 conformance fixtures (2.3 §Conformance).

## 10. Early boot and stderr

> **LOG8 — Early-boot output.** Before the config layer stack is
> loaded (boot stages 0..1), rendered lines go to stderr and into
> the ring. Once the sink opens, it replays the ring head; stderr
> output then stops for client runtimes that own a terminal (a TUI
> never writes log lines to its controlling terminal after raw mode
> begins) and continues for headless server runtimes only if
> `kernel.host.[log].stderr = true` (default: true for servers,
> false for terminal clients).
> *Class*: runtime.

## 11. Verbosity flags

`-v` / `-vv` and `--log <PATH>` are launcher sugar for layer-6 CLI
config (1.5 §3): `-v` ⇒ `--set kernel.host.log.level=debug`, `-vv` ⇒
`…=trace`, `--log <PATH>` ⇒ `--set kernel.host.log.path=<PATH>`.
They introduce no parallel mechanism. Rule `AL11` in 1.3 §8 owns the
flag surface.

## Open items

1. Curated render templates for high-traffic structured families
   (vs the default `key=value` rendering). Draft: default rendering
   only; templates are additive later.
2. Whether `hostapi_log_emit` gains a structured-fields variant for
   ext code. Draft: no for ABI major 1; kernel-owned families only.
3. JSON-lines sink variant. Draft: out of scope — JSON consumers
   subscribe to DS12 directly (7.2); the file sink is for humans.

## Conformance

| Rule | Fixture |
|---|---|
| LOG1 | Every sink line pairs 1:1 with an event seen by a parallel DS12 subscriber; no extra lines. |
| LOG2 | Golden test: fixed event fixtures render byte-identical lines (timestamp width, escaping, instance grammar §4). |
| LOG3 | Multi-vtable cdylib emits from two vtables → `kind/id.0` and `kind/id.1`; caller-supplied identity in `target` does not alter `emitter`/`address`; kernel event renders a bare subsystem address. |
| LOG4 | `hostapi_log_emit` produces a `log.message` event; oversized message truncates at `max-log-message-bytes`; flood coalesces per OBS rate limits. |
| LOG5 | Slice ownership: caller frees its buffers after return; host copy survives. `ResourceExhausted` on rate-limit breach. |
| LOG6 | Ring wraps oldest-first on overflow; `hostapi_debug_log_ring_read` returns the most recent lines; boot-stage-0 lines present before the first cdylib load. |
| LOG7 | Rotation at `rotate-bytes` produces `.1`; `rotate-keep` bound holds; sink failure (unwritable path) leaves editing unaffected and emits `log.sink.fail`. |
| LOG8 | Late sink open replays boot lines; TUI fixture: no log bytes on the controlling terminal after raw mode. |
