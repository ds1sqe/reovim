# 7.2 — Debug Surface

**Scope.** The debug channel: events, drive (state mutation), the
auth boundary, what's privileged and what's read-only.

**Heritage.** v3 `07-Surfaces/02-Debug-Surface.md`; v4 README §15.

**Locked rules.** `DS1..DS12` carried (restated §2; DS5 reshaped),
`DS13` new.

---

## 1. Surface

Debug exposes:

- **read** — DS12 events (kernel structured events; see 9.4),
  inventory snapshots, frame captures, projection diffs.
- **drive** — controlled state mutation: send `RawInput`, force
  re-projection, allocate/release view-slots, etc.

Both flow through the framed protocol (7.3) on a separate
namespace from the user session protocol.

## 2. Carried rules (DS1..DS12)

The v3 rule bodies, restated in v4 vocabulary. These are the
normative texts; the v3 chapter is heritage.

> **DS1 — Each input modality is its own channel.** Key, mouse,
> IME, trigger, … (the 6.3 §7 `RawInputKind` catalog) are separate
> debug channels, not collapsed into one `pointer` channel. Schema
> and capabilities live per modality. *Class*: spec-asserted.

> **DS2 — Extension channels follow `x-<vendor>-<name>` naming**
> (HTTP-header style). Vendor-uniqueness collisions surface as
> packaging conflicts at `pkg sync`, not in the kernel. *Class*:
> tooling.

> **DS3 — JSON at the CLI boundary; the framed-protocol envelope
> carries channel payloads as opaque bytes.** The CLI translates at
> the edge using a hand-written JSON reader/writer in-repo,
> validating against the channel's JSON Schema before encoding.
> Channel **payload** schemas are JSON-Schema-defined; the
> framed-protocol **envelope** (§6, 7.3) carries them as opaque
> bytes. *Class*: tooling + runtime.

> **DS4 — The kernel multiplexes observe subscribers** (default
> capacity 64); producers see one subscription per channel.
> Per-subscriber backpressure with channel-typed drop policy:
> `state` coalesces newest-wins, `frame` drops intermediates,
> `input` drop-newest with a `dropped: N` marker. Blocking
> subscriptions are opt-in and accept producer-side backpressure.
> *Class*: kernel-enforced (runtime).

> **DS5 (reshaped) — Transport auth + the coarse capability
> pair; nothing finer.** Authentication is transport-layer only
> (Unix-socket perms locally; mTLS/token on TCP remotely, SEC2).
> Authorization is the two-capability vocabulary of §5.1
> (`debug.read`, `debug.mutate` per DS13). There is no per-channel
> ACL and no per-channel capability token. *Class*: runtime.

> **DS6 — Recording format is JSONL.** One self-contained JSON
> event per line with at minimum `{ts, channel, payload}`. A
> future `--format binary` flag may be added; the default does not
> change. *Class*: tooling.

> **DS7 — `cli drive --stdin --timing {asap|realtime|paced}`.**
> ASAP is default; realtime honours recorded `ts`; paced is
> interactive (one event per Enter). Per-call flag; recordings
> carry timestamps either way. *Class*: tooling.

> **DS8 — The kernel is the capability source of truth.** The CLI
> checks capabilities before sending when reachable and errors
> locally on unsupported channels with the supported list for
> context; when unreachable it passes through and the kernel
> returns the canonical `ErrorCode::NotFound`. The CLI cache is
> best-effort. *Class*: tooling + runtime.

> **DS9 — Inventory enumeration reads kernel state only.** The
> inventory snapshot (§5; kernel inventory RPC) lists loaded cdylibs
> with `{name, kind: ManifestKind, abi: AbiVersion, status,
> refcount, manifest_path}`. No `dlopen`, no cdylib involvement.
> Tombstoned lifecycle states (2.2) appear with their status.
> *Class*: kernel-enforced (runtime).

> **DS10 — Buffer description returns a `DomainTreeSnapshot`.**
> One node per `DomainAttachment`:
> `{id, domain_id, scope, parent_id, struct_refs, focus_refs}`
> (DT11 adds `struct_refs` to the v3 set). View-state contents are
> NOT included (opaque per DT3); a separate handler-query op
> inspects slots. Buffer references carry `buffer_id` only.
> *Class*: kernel-enforced (runtime).

> **DS11 — `DebugCaps` declares the channel surface.**
> `DebugCaps { kind: ManifestKind, version, channels: Vec<ChannelDecl> }`;
> `ChannelDecl { tag, schema_url, class: ChannelClass, drop_policy: DropPolicy }`.
> There is no redundant `Direction` field — `ChannelClass` carries
> direction by class. *Class*: spec-asserted.

> **DS12 — `kernel_log` is the canonical kernel-emitted observe
> channel.** Event shape `{ts, level, event, fields}`; required
> event families are enumerated in 9.4 (OBS1) with schemas locked
> per OBS3. `hostapi_log` is the cdylib write path into
> `kernel_log`; kernel emitters write directly. Drop policy:
> drop-newest with a `dropped: N` marker. *Class*:
> kernel-enforced (runtime).

**Reshape note (DS1..DS12).** Carried from v3 with v4 vocabulary.
DS5 is the one reshape: v3's "no app-level ACL, no capability
tokens" is superseded by the §5.1 capability pair — DS13 layers
`debug.mutate` on top of transport auth, and the rule now reads
"nothing finer than the pair". DS3's "JSON Schema is the source of
framed-protocol carries channel payloads as opaque bytes" is the v4
model; CF5 cross-checks chapter ↔ message-struct alignment. DS8's v3 `NotSupported` answer maps to
`ErrorCode::NotFound` (AB14: one error vocabulary). DS9/DS10 field
sets are updated to the v4 lifecycle states and the DT11 dual
refcounts. DS12's event families moved to 9.4 (OBS1/OBS3); the
channel mechanics stay here.

## 3. Drive privileges

> **DS13 — Debug-drive requires explicit mutate capability beyond
> transport auth.** A client invoking `drive_*` operations on the
> debug surface MUST carry an explicit `debug.mutate` capability in
> addition to whatever transport auth was satisfied. Each
> drive operation emits a `debug.drive.start|ok|fail` audit event
> (correlation-ID-tagged).
>
> *Class*: runtime.

The capability comes from the user-session attach negotiation; the
launcher refuses to start `drive` mode unless it has been granted.

## 4. Drive surface

```c
hostapi_debug_drive_input    (ClientId, BufferId, WindowId, RawInput);
hostapi_debug_drive_attach   (BufferId, ParentAttachment, DomainId, ...);
hostapi_debug_drive_unload   (CdylibId);
hostapi_debug_drive_force_unload (CdylibId);     // bypasses drain (FAIL3-warned)
hostapi_debug_drive_log_ring_clear ();           // LOG11 (7.5): dmesg -C analog
hostapi_debug_drive_shutdown ();                 // LF14 (2.2 §9): graceful shutdown trigger
```

Force-unload is a developer escape hatch with FAIL3 violation
logged.

## 5. Read surface

```c
hostapi_debug_inventory_snapshot      (InventorySnapshot* out);
hostapi_debug_event_subscribe         (EventFilter, SubscriberHandle* out);
hostapi_debug_event_unsubscribe       (SubscriberHandle);
hostapi_debug_capture_frame           (ClientId, BufferId, WindowId, FrameBlob* out);
hostapi_debug_capture_projection_diff (ClientId, BufferId, WindowId, DiffBlob* out);
hostapi_debug_log_ring_read           (ByteBuf* out);   // LOG6 (9.5): dmesg analog
hostapi_debug_buffer_bytes_read       (BufferId, u64 offset, u64 len, ByteBuf* out);
                                                        // DEV4 (10.1): state-golden reads
```

Read operations require transport auth plus the `debug.read`
capability declared at attach (§5.1). They never mutate state.

### 5.1 Debug capability vocabulary

Two capability strings cover the debug surface; both are declared
in `ClientCapabilities.caps` at attach (7.3 §7):

| Capability | Grants |
|---|---|
| `debug.read` | event subscription, inventory snapshots, frame / projection capture |
| `debug.mutate` | all `drive_*` operations (DS13); implies nothing about read — declare both for a full debug client |

Future refinements (`debug.replay`, `debug.fuzz`) are additive
capability strings, never repurposings of these two.

## 6. Capability mediation

The client-side debug capability is a **sibling vtable** in the
client cdylib (per CL4). The server-side debug surface routes
opaque payloads but does not decode capability semantics (CL5).

Wire shape (framed-protocol message structs; see 7.3 and 6.3):

```rust
struct DebugDriveOp { payload: ByteSlice, op_kind: u32 }
struct DebugReadOp  { payload: ByteSlice, op_kind: u32 }
```

Decoding happens at the client capability boundary, not the server.

## 7. Transport boundary

`auth = "none"` is local-only (SEC2). Remote debug:

- `tcp` framed-protocol transport with `auth = "mtls"` or `auth = "token"`,
- `debug.mutate` capability required for drive,
- audit events `debug.drive.*` per operation.

## Open items

1. Capability allowlist syntax for non-debug capabilities
   (`session.attach`, render capabilities, ...) — the debug pair
   is fixed in §5.1; the full session-capability vocabulary
   belongs to 7.3.
2. Whether drive operations are rate-limited at the transport level.
   Default: yes, configurable in `kernel.host.[limits]`.
3. Replay of recorded debug streams — out of v4 target as a feature,
   but the schema must not foreclose it.

## Conformance

| Rule | Fixture |
|---|---|
| DS2 | Two packages declare `x-acme-trace` → conflict surfaced at `pkg sync`. |
| DS4 | Slow subscriber on an `input` channel → drop-newest with `dropped: N` marker; `state` channel coalesces. |
| DS5 | Remote TCP connection without token → rejected at transport; no channel-level grant can substitute. |
| DS8 | CLI offline-cache stale; kernel returns `NotFound` for a removed channel; CLI surfaces it canonically. |
| DS9 | Snapshot during a load shows the loading cdylib's status; tombstoned entry visible after failed unload. |
| DS10 | Snapshot contains both refcounts and no view-state bytes. |
| DS12 | Cdylib `hostapi_log` event arrives on `kernel_log` matching the locked schema (OBS3). |
| DS13 | Client without `debug.mutate` calls drive op → rejected; audit event recorded. |
| (read-only) | Client with read-only auth subscribes to events → succeeds. |
| (mutate audit) | Drive op succeeds → `debug.drive.ok` event with correlation ID. |
