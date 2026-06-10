# 7.3 — Server-Client Protocol

**Scope.** gRPC v3 protocol surface, message inventory, attach
flow, capability negotiation, backpressure.

**Heritage.** v3 `07-Surfaces/03-Server-Client-Protocol.md`; v4
README §0.4 (provenance for generated `.proto`).

**Locked rules.** `SP1..SP7` carried (restated §9); `SP8` new
(§10).

---

## 1. Wire format

gRPC v3. Proto files at `uapi/protocol/proto/`. Ownership is
split (1.4 §1): the `.proto` files own the **wire form**
— field numbers, message shapes, service signatures — and are
edited as source; this chapter owns the **semantics** —
capability negotiation, error mapping, attach flow, audit
behaviour. CI cross-checks the two (CF5): every message and RPC
named here must exist in a `.proto` with matching field numbers,
and proto-side removals are flagged as breaks.

## 2. Service surface (high level)

```proto
service Reovim {
    rpc Attach          (AttachRequest)        returns (stream AttachEvent);
    rpc SendInput       (stream InputBatch)    returns (stream InputAck);
    rpc Detach          (DetachRequest)        returns (DetachAck);

    rpc SwitchSession   (SwitchSessionRequest)  returns (SwitchSessionAck);
    rpc DestroySession  (DestroySessionRequest) returns (DestroySessionAck);
    rpc RenameSession   (RenameSessionRequest)  returns (RenameSessionAck);

    rpc DebugRead       (DebugReadRequest)     returns (stream DebugReadEvent);
    rpc DebugDrive      (stream DebugDriveOp)  returns (stream DebugDriveAck);

    rpc PkgSync         (PkgSyncRequest)       returns (stream PkgSyncEvent);
    rpc PkgVerify       (PkgVerifyRequest)     returns (PkgVerifyAck);

    rpc ConfigDump      (ConfigDumpRequest)    returns (ConfigDumpResponse);
    rpc ConfigValidate  (ConfigValidateRequest) returns (ConfigValidateResponse);
}
```

(Full message inventory pending — flagged in v4 README §0.4.)

## 3. Attach flow

```
1. client Attach(request) — declares ClientCapabilities + auth
2. server validates auth + capabilities
3. server emits AttachEvent::Ack with:
   - DomainTable (CR9): list of (domain_name, inner_id, displayable, semantic)
   - assigned ClientId
4. server streams AttachEvent::Frame / Diff / Cursor / Projection updates
5. server emits AttachEvent::DomainTableDelta on codec register/unregister
```

## 4. DomainTable

```proto
message DomainTable {
    repeated DomainEntry entries = 1;
}
message DomainEntry {
    string  domain_name  = 1;        // stable name (CR9)
    uint32  inner_id     = 2;
    bool    has_display  = 3;
    bool    has_semantic = 4;
}
```

A client lacking a local codec for a `(name, inner_id)` follows
CR10 fallback.

## 5. Cursor / position transport

Cursor blobs travel as base64-encoded bytes within proto messages,
with `domain_name` for stable identity remap.

```proto
message CursorBlob {
    string  domain_name = 1;
    uint32  inner_id    = 2;
    uint32  flags       = 3;
    bytes   content     = 4;
}
message PositionBlob {
    string  domain_name = 1;
    uint32  inner_id    = 2;
    uint32  flags       = 3;
    bytes   content     = 4;
}
```

Empty cursor sets are zero-length repeated fields (CR3 — no
sentinel).

## 6. Backpressure

- Per-stream window from `kernel.host.[limits].grpc-*`.
- Server applies CR-style flow control on attach frame output.
- Client `SendInput` rate-limited per CR15-style cap on the input
  side.

## 7. Capability negotiation (SP6)

Client declares its capability set at `Attach`:

```proto
message ClientCapabilities {
    repeated string  caps           = 1;      // "render.cells", "render.svg", ...
    repeated string  domain_codecs  = 2;      // "text.utf8", "hex.x16"
    uint32           protocol_minor = 3;
}
```

Server compares against `kernel.host.[limits].required-caps`. A
client missing required caps is rejected with `ProtocolViolation`.

## 8. Protocol versioning

- `protocol_minor` is the gRPC contract minor.
- Major bump = a new `ReovimV2` service **served alongside**
  `Reovim` — the shipped service is never removed or replaced
  (AB15: released wire surfaces are eternal).
- Minor adds messages / fields; old clients ignore unknown fields.
- Shipped field numbers are never reused or repurposed.

## 9. Carried rules (SP1..SP7)

The v3 rule bodies, restated in v4 vocabulary. These are the
normative texts; the v3 chapter is heritage.

> **SP1 — Single-stream-per-client.** The server rejects a second
> `Attach` on the same transport connection with
> `FAILED_PRECONDITION`. *Class*: kernel-enforced (runtime).

> **SP2 — `SwitchSession` pivots in place.** The server emits
> `SessionPivot` on the existing `Attach` stream followed by the
> new session's initial-projection sequence. No reconnect.
> *Class*: kernel-enforced (runtime).

> **SP3 — Graceful stream close is implicit detach.** The server
> reaps the client — focus chains release through the LF12
> teardown ordering — and emits
> `ClientLeft { reason: "stream-closed" }` to remaining clients.
> Explicit `Detach` exists for telemetry / DS6-recording reasons.
> *Class*: kernel-enforced (runtime).

> **SP4 — `DestroySession` is broadcast.** Every attached client
> receives `SessionDestroyed`; their streams terminate with OK
> status, not an error. *Class*: kernel-enforced (runtime).

> **SP5 — `RenameSession` migrates persisted state eagerly.** The
> rename updates both the user-visible name and
> `SessionId(Arc<str>)`. Persisted state keyed by the old name
> (2.4 §2, `state/sessions/<session-id>/`) migrates as part of the
> rename, not lazily. *Class*: kernel-enforced (runtime).

> **SP6 — No `CreateClient` RPC.** Client identity is the
> transport connection; the first RPC registers the client.
> Capability declaration rides `Attach` (§7). *Class*:
> spec-asserted.

> **SP7 — No `KickClient` RPC in v1.** `DestroySession` with
> `force = true` is the only authorised tear-down path; per-client
> admin operations are a follow-on protocol minor. *Class*:
> spec-asserted.

The RPC inventory itself is contract surface, not a numbered rule:
adding, renaming, or removing an RPC is a protocol major — which
per §8 and AB15 means a new service served alongside, never an
edit to the shipped one.

**Reshape note (SP1..SP7).** Semantics unchanged from v3. v3's
`AttachSession` / `DetachSession` RPC names are v4's `Attach` /
`Detach`; the v3 8-RPC inventory grew (debug, pkg, config RPCs)
under the same inventory-change-is-major rule; wire-form ownership
moved to the `.proto` files per CF5 — these rules state
semantics only.

## 10. Shutdown notification

> **SP8 — `ServerDraining` precedes shutdown detach.** At LF15
> phase 1 (2.2 §9), the server sends
> `ServerDraining { grace_ms }` on every attached client's
> `Attach` stream. Clients SHOULD detach voluntarily within the
> grace window; phase 2 detaches the remainder. Streams closed by
> shutdown terminate with OK status (a clean end, not an error);
> a new `Attach` after shutdown begins is rejected `UNAVAILABLE`
> (hostapi side: `ErrorCode::Busy`, LF15 phase 0). The message is
> additive on the `AttachEvent` stream — a protocol minor.
> *Class*: runtime.

## Open items

1. Full message inventory per RPC (4..6 messages each).
2. ~~Provenance~~ — resolved (§1, CF5): proto owns wire
   form, chapter owns semantics, CI cross-check.
3. Multi-client attach to one session — locking and ordering.
4. Streaming-input vs request/response for `SendInput` — currently
   bidi; confirm.

## Conformance

| Rule | Fixture |
|---|---|
| SP1 | Second `Attach` on one connection → `FAILED_PRECONDITION`; first stream unaffected. |
| SP2 | `SwitchSession` mid-stream → `SessionPivot` then full initial projection; no transport reconnect observed. |
| SP3 | Kill client transport gracefully → server emits `ClientLeft{stream-closed}`; focus chains released. |
| SP4 | `DestroySession` with two clients attached → both receive `SessionDestroyed`; both streams end OK. |
| SP5 | Rename session, restart server → state restores under the new name; old-name dir gone. |
| SP6 | Fresh connection's first RPC registers the client; no separate create call exists in the proto (CF5 check). |
| SP7 | Proto surface contains no `KickClient` (CF5 check); `force=true` destroy tears down an unresponsive client. |
| SP8 | Shutdown with an attached client → `ServerDraining` observed before stream end; stream terminates OK; `Attach` after shutdown begins → `UNAVAILABLE`. |
| (attach flow) | Client without required caps → ProtocolViolation. |
| (DomainTable delta) | Codec registers post-attach → server emits delta. |
| (CR10 hop) | Client sees Domain in table without local codec → fallback path. |
