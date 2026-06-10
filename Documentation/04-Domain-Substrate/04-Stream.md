# 4.4 — Stream Substrate

**Scope.** The kernel's substrate for live byte streams: registered
schemes, stream lifecycle, backpressure, drain, cdylib-owned scheme
implementations.

**Heritage.** v3 `04-Substrate/04-Stream.md` (S1..S10 + L26 retained);
v4 README §0.4 mentions `driver-stream` cleanup.

**Locked rules.** `S1..S10` carried (restated §12); `L26` reshaped
(Stream replaces the v3 single-purpose terminal carve-out).

---

## 1. Substrate model

A **stream scheme** is a cdylib that registers a runtime-loaded byte
producer/consumer. The kernel does not bake terminal, file watcher,
or process IO — each ships as a stream-scheme cdylib.

```rust
pub struct StreamScheme {
    pub name:             Arc<str>,           // e.g. "pty", "watch", "child"
    pub vtable:           StreamSchemeVtable,
    pub owner_cdylib_id:  CdylibId,
    pub api_version:      Version,
}

pub struct StreamHandle {
    pub id:               StreamId,
    pub scheme:           Arc<str>,
    pub state:            StreamState,
    pub session_id:       Option<SessionId>,
    pub buffer_id:        Option<BufferId>,
    pub backpressure:     BackpressureCounters,
}

pub enum StreamState {
    Init,
    Running,
    BackpressureBlocked,
    Stale,             // underlying source ended; awaiting drain
    Draining,
    Closed,
}
```

## 2. Lifecycle (S1..S5)

```
1. open:    scheme.open(args) → StreamId
2. read:    bytes flow into kernel from scheme producer
3. backpressure: kernel applies BP rules per S6/S7
4. close:   scheme.close(StreamId) → drain → Closed
5. unload:  on cdylib unload, all streams it owns transition to
            Draining and complete or time out per FAIL3
```

## 3. Scheme registration

```c
ErrorCode hostapi_stream_scheme_register(const StreamScheme* scheme);
```

Registration runs during `init`; scheme rows are tracked in
`Inventory.rows_by_owner`.

The scheme vtable (sketch; AB14 — every fallible slot returns
`ErrorCode`):

```c
typedef struct {
    VtableHeader header;
    ErrorCode (*open)   (const StreamOpts*, StreamId* out);
    ErrorCode (*write)  (StreamId, ByteSlice);             // S4: non-blocking
    ErrorCode (*control)(StreamId, uint32_t op,
                         ByteSlice arg, ByteBuf* out);     // S5: typed ioctl, §10
    ErrorCode (*close)  (StreamId);                        // S8: drains
} StreamSchemeVtable;
```

```rust
#[repr(C)]
pub struct StreamOpts {
    pub url:         ByteSlice,   // "pty://...", routed by scheme name
    pub scheme_opts: ByteSlice,   // typed payload per S10 (§11)
}
```

## 4. Backpressure (S6, S7)

The kernel tracks per-stream:

```rust
pub struct BackpressureCounters {
    pub bytes_in_flight:  u64,
    pub bytes_buffered:   u64,
    pub last_drain_at:    Instant,
}
```

When `bytes_in_flight` exceeds the per-stream cap, the kernel
returns `Blocked` to the scheme; the scheme is responsible for
honouring backpressure (sleep / not-ready / Suspend, depending on
its model).

Caps from `kernel.host.[limits]`:

| Field | Default |
|---|---|
| `stream-max-bytes-in-flight` | 8 MiB |
| `stream-max-bytes-buffered` | 16 MiB |
| `stream-drain-timeout-ms` | 3000 |

Ownership of the counted bytes: `bytes_in_flight` counts bytes
accepted by `hostapi_stream_emit` but not yet applied to buffers
(scheme-originated, kernel-copied per S2); `bytes_buffered` counts
the kernel-owned per-subscription queues (S3). Pre-emit source
buffering inside the scheme is the scheme's own memory and is not
kernel-accounted.

## 5. Stale state (S8)

When a stream's underlying source ends (PTY child exits, watcher
cancelled), the scheme transitions the handle to `Stale` and stops
producing new bytes. Existing buffered bytes remain readable.
Consumers see `Stale` after the last byte and may close.

## 6. L26 reshape

> **L26 — The kernel's stream surface is the substrate, not a
> baked terminal.** Concrete schemes (`pty`, `watch`, `child`,
> future schemes) ship as cdylibs that register at participant
> init. The substrate provides the lifecycle, BP, drain, and
> ownership infrastructure; per-scheme semantics are the cdylib's
> policy. *Class*: spec-asserted.

PTY is the v1.1 reference scheme. Q-6 (terminal emulation) is
resolved by this rule.

## 7. Manifest kind

Stream-scheme cdylibs use `ManifestKind::StreamScheme` (manifest
string `stream-scheme`) — their **own kind**, not a driver or
provider sub-kind. The substrate (S1..S10) is its own contract:
schemes do not implement a subsys driver trait, and forcing them
under `driver-*` would misstate the layer they plug into. Concrete
scheme names (`pty`, `watch`, `child`) are sub-kinds discriminated
by the scheme's registered name, mirroring how driver sub-kinds
discriminate by vtable kind string (6.2 §7).

Manifests live at `manifest/stream/<name>.toml` with vtable kind
`stream_scheme` and symbol `REOVIM_STREAM_SCHEME_VTABLE`.

## 8. Bounded resources

| Cap | Field |
|---|---|
| Max streams per session | `max-streams-per-session` |
| Max schemes per process | `max-stream-schemes` |

## 9. Reverse flow

A stream scheme calls back into HostApi to:
- write bytes into a buffer or session-side sink,
- emit DS12 events keyed by stream identity,
- request backpressure status,
- declare stream stale.

Reverse-flow lock discipline is per CC16.

## 10. Control plane (S5)

`control` is the typed-ioctl escape hatch. The `u32` op space is
partitioned:

| Range | Owner |
|---|---|
| `0` | reserved-invalid (rejected with `InvalidArgument`; mirrors `ManifestKind::Unknown = 0`) |
| `1..=100` | kernel-defined **stream-protocol ops only** — kind-agnostic (`Inspect`, `Drain`, future) |
| `101..=255` | scheme-private |

Stream-kind vocabulary (PTY resize, POSIX signal, inotify mask,
socket linger, …) MUST live in the scheme-private range — the
kernel does not know what a terminal resize is. The kernel never
originates an op in `101..=255`; that range is used only by
modules that know the target scheme's vendor extension. This is
Plan9-faithful, not Linux-faithful (Linux's ioctl space is
genuinely kind-aware) — an intentional divergence that keeps the
kernel mechanism-only. The kernel op range evolves append-only
per the type-catalog stability rules (6.3 §11; AB15 post-v1.0).

## 11. Scheme options (S10)

The `StreamOpts.scheme_opts` payload schema MUST be a `#[repr(C)]`
record published in the scheme's manifest as
`scheme_opts_schema = "<typename>"` (`manifest/stream/<name>.toml`,
§7). The named type must be defined in a uapi crate the scheme's
consumers can link against. Ad-hoc string-soup parsing (TOML,
JSON, query-strings) inside `scheme_opts` is forbidden — the
opaque-payload escape hatch is for typed FFI-safe records, not
unstructured text.

## 12. Carried rules (S1..S10)

The v3 rule bodies, restated in v4 vocabulary. These are the
normative texts; the v3 chapter is heritage.

> **S1 — Stream schemes are their own cdylib kind.** One vtable
> contract (`StreamSchemeVtable`, symbol
> `REOVIM_STREAM_SCHEME_VTABLE`, `ManifestKind::StreamScheme` per
> §7); many implementations. Schemes MUST NOT opt into per-cdylib
> serialization (`requires_serialization`, PM9 / 7.1) — emit
> re-entrancy would deadlock the serialization mutex. A stream
> manifest declaring it `true` is rejected at participant init
> with `ErrorCode::Conflict` plus a DS12 `kernel_log` error.
> *Class*: kernel-enforced (compile + LF).

> **S2 — Stream bytes enter buffers through the byte-edit path.**
> Bytes flow via `ByteEdit { origin: External }` through the
> HostApi buffer-apply-edit call — kernel-mediated, never
> vtable-to-vtable (6.1 §6). DT6 observation fan-out applies
> unchanged. The payload is **copy-at-boundary** (6.2 §4): the
> kernel allocates and copies inside `hostapi_stream_emit`; the
> scheme may free its source bytes on return. This forecloses any
> "Domain writes view-state without going through buffer bytes"
> pattern. *Class*: kernel-enforced (runtime).

> **S3 — Subscriptions are many-buffers-per-stream; pacing is
> consumer-shaped.** The scheme fans out at emit time. Each
> subscription carries its own `SubscribeOpts` (`coalesce_ms`,
> `max_buffered_bytes`) — pacing belongs to the consumer, not the
> source. The per-subscription queue is **kernel-owned** (counted
> by `bytes_buffered`, §4). *Class*: spec-asserted.

> **S4 — `write` is op-on-stream, not op-on-buffer.**
> Bidirectional streams accept input via `vtable.write`; the
> kernel does NOT auto-forward buffer edits back to the stream
> source. `write` is non-blocking by contract: returns
> `ErrorCode::Busy` immediately if the source isn't ready,
> `ErrorCode::Stale` if the peer has hung up. *Class*:
> spec-asserted.

> **S5 — `control` is the typed-ioctl escape hatch.** Op space
> partitioned per §10: `0` reserved-invalid, `1..=100`
> kernel-defined kind-agnostic, `101..=255` scheme-private.
> *Class*: spec-asserted.

> **S6 — Scheme-name registration is unique.** Two cdylibs
> registering the same scheme name conflict; the second fails at
> participant init with `ErrorCode::Conflict`. *Class*:
> kernel-enforced (LF).

> **S7 — Push delivery; the kernel does not poll.** Schemes call
> `hostapi_stream_emit` from their own runtime/thread. The
> kernel-side emit thunk wraps fan-out in `catch_unwind` (AB12);
> on panic it returns `ErrorCode::Panic` and does NOT poison
> buffer locks. *Class*: kernel-enforced (runtime).

> **S8 — `close` drains; `Stale` is the after-close answer.**
> `vtable.close` drains in-flight emits before returning. After
> close, an emit against the closed stream's subscriptions returns
> `ErrorCode::Stale`. Detach-during-emit (a subscribed buffer
> detaches mid-emit) auto-drops the subscription and returns
> `Stale`; schemes tolerate `Stale` as implicit unsubscribe.
> Unsubscribe is idempotent. *Class*: kernel-enforced (runtime).

> **S9 — Schemes iterate their subscribers.** One emit call per
> subscription in the scheme's own fan-out loop; the kernel does
> NOT iterate subscribers. Fan-out is best-effort per
> subscription: partial failure does not roll back successful
> applies; a failed apply emits a DS12 `kernel_log` warning.
> *Class*: spec-asserted.

> **S10 — Scheme options are typed.** `scheme_opts` schema rules
> per §11. *Class*: spec-asserted.

**Reshape note (S1..S10).** Carried from v3 with the
StreamDriver→StreamScheme rename throughout: schemes are their own
`ManifestKind` (§7), no longer a driver sub-kind; the symbol is
`REOVIM_STREAM_SCHEME_VTABLE`; `driver_opts` became `scheme_opts`.
v3's opaque `NonZeroU64` handle became the kernel-side
`StreamHandle` with an explicit `StreamState` machine (§1) — v3's
generation-mismatch semantics surface as the `Stale` variant.
S1's serialization reference is now the PM9
`requires_serialization` manifest flag (7.1). S4's
`WouldBlock`/`Closed` returns map to `ErrorCode::Busy`/`Stale`
(AB14: one error vocabulary). v3's AB10 append-only-enum rule for
the control-op space is subsumed by the type-catalog stability
rules + AB15. No rule in the block is dropped.

## Open items

1. ~~Manifest kind name~~ — resolved (§7):
   `ManifestKind::StreamScheme` / `stream-scheme`.
2. Whether stream schemes can register dynamically post-init or
   only during init. Default: only during init (consistency with
   handler/projector registration).
3. ~~Buffered-byte ownership~~ — resolved (§4, S3 restatement):
   per-subscription queues are kernel-owned (`bytes_buffered`);
   pre-emit source buffering is scheme-owned and not
   kernel-accounted.

## Conformance

| Rule | Fixture |
|---|---|
| S1 | Stream manifest with `requires_serialization = true` → rejected at init with `Conflict` + DS12 error. |
| S2 | Emit 1 MiB; scheme frees source bytes on return; buffer content intact (kernel copy). |
| S3 | Two subscriptions with different `coalesce_ms` on one stream → each paced independently. |
| S4 | `write` to a not-ready source returns `Busy` without blocking; to a hung-up peer returns `Stale`. |
| S5 | `control(op=0)` → `InvalidArgument`; kernel never emits an op in `101..=255` (trace probe). |
| S6 | Two cdylibs register scheme `pty` → second fails init with `Conflict`. |
| S7 | Panicking fan-out → emit returns `Panic`; subsequent buffer ops succeed (no poisoned lock). |
| S8 | Emit after close → `Stale`; double unsubscribe → second call is a no-op success. |
| S9 | One of three subscription applies fails → other two buffers updated; DS12 warn emitted. |
| S10 | Stream manifest without `scheme_opts_schema` while `scheme_opts` non-empty → `pkg sync` rejects. |
| L26 | PTY scheme cdylib registers `pty` scheme; kernel has no PTY-specific code path. |
| (BP) | Stream emits at 200 MiB/s with consumer at 1 MiB/s; verify scheme sees `Blocked` and bytes-in-flight stays under cap. |
| (Stale) | Child process exit → handle goes Stale; consumers see Stale after final byte. |
| (Unload) | Unload scheme cdylib while streams open → all handles drained per FAIL3. |
