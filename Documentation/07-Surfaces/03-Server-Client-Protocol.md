# 7.3 — Server-Client Protocol

**Scope.** The in-house **framed wire protocol**: transport, frame
layout, the deterministic byte codec, handshake/versioning, the full
message inventory, request/response correlation, the notification
stream, the reject/error model, backpressure, and multi-client
semantics. This chapter is the wire truth: a stranger MUST be able to
reimplement a conforming client from this chapter alone (plus the
`#[repr(C)]` layouts it cites in 6.3).

**Locked rules.** `SP1..SP8` carried (restated §11, semantics
unchanged, wire-mechanism vocabulary reshaped); `SP9..SP14` new
(framing, handshake, codec, reject model, unknown-tag policy,
backpressure); `SP15..SP16` new (sans-IO protocol purity, carrier
replaceability — §1a).

> Heritage (non-normative). Earlier drafts specified this surface as a gRPC
> service: `service Reovim { rpc … }` over HTTP/2, with `.proto`
> message definitions owning the wire form and prost-generated
> bindings on each side. Cursor and position blobs travelled as
> base64-encoded `bytes` proto fields. Dependency Sovereignty (L9 /
> DAG5, 1.2 §9) closes the dependency graph to `std`/`core`/`alloc` +
> in-repo code; a third-party RPC stack, a proto compiler, and a
> base64 crate are all forbidden. The gRPC form is therefore heritage.
> This chapter replaces it with a hand-written framed protocol over a
> raw stream socket. The mapping of every gRPC concept to its framed
> successor is in the `> Heritage` notes throughout.

---

## 1. Design rationale (fastest-reaction)

reovim is the fastest-reaction editor (README §North Star). The wire
between client and server is on the hot path of every keystroke. Three
choices serve latency directly:

- **UDS primary.** Local clients connect over a Unix-domain stream
  socket. No TCP/IP stack, no loopback checksum, no Nagle interplay —
  the kernel copies bytes between two processes on the same host. TCP
  is the secondary, opt-in remote transport.
- **Raw length-prefixed frames.** A frame is a 16-byte fixed header
  plus a codec-encoded body. There is no HTTP/2 framing layer, no
  stream-multiplexing state machine, no header compression table.
- **Zero codec indirection.** Every message is a hand-written struct
  with a direct `encode`/`decode` pair against the same deterministic
  byte codec the 6.3 `*Wire` catalog already uses. Cursor and position
  payloads are raw length-prefixed bytes — no text-expansion encoding,
  no schema-driven reflection.

> Heritage (non-normative). The gRPC form paid for HTTP/2 framing,
> protobuf field-tag varints, and base64 cursor expansion on every
> frame. The framed protocol removes all three indirections.

## 1a. Protocol purity and the carrier seam

This chapter specifies **two layers** with a hard seam between them:

- The **message protocol** — the deterministic byte codec (§6), the
  message inventory (§7), the handshake/versioning rules (§10), the
  correlation rules (§4), and the reject model (§9). This layer is
  pure computation over byte slices. It is the protocol SSOT.
- The **carrier** — whatever moves the bytes. §§2–3 specify the
  **default carrier**: length-prefixed frames over a UDS/TCP stream
  socket. The default carrier is normative for interoperability, but
  it is *one* carrier, not the protocol itself.

> **SP15 — `uapi/protocol` is sans-IO pure.** The codec, message
> structs, and handshake/correlation state machine perform ZERO IO:
> no syscalls, no `arch/` IO APIs, no IO traits, no transport
> assumptions, no timers, and no allocation (callers provide every
> buffer; 1.2 §10). The encode/decode surface is pure over byte
> slices: `encode(&self, &mut [u8]) -> Result<usize, ErrorCode>` /
> `decode(&[u8]) -> Result<Self, ErrorCode>`. Purity is
> probe-enforced: `uapi/protocol` imports nothing but `core` and
> `uapi` siblings. *Class*: CI/depgraph (purity probe) + spec.

> **SP16 — The carrier is a replaceable byte-mover; framed UDS/TCP
> is the default.** Any mechanism that satisfies the carrier
> contract below is a valid carrier: the §2 stream socket, HTTP,
> WebSocket, gRPC, named pipes, even plain file IO. The message
> protocol cannot tell the difference, and a conforming
> implementation keeps it that way. Every carrier implementation
> honors `DAG5`/`DAG6` (in-house code, no third-party crates,
> `no_std` over `arch/` APIs). *Class*: spec-asserted + structural.

**Carrier contract.** A carrier MUST provide, per connection:

- ordered, reliable, complete delivery of frame bytes in each
  direction (no loss, no duplication, no reordering within the
  connection);
- a connection identity — the connection IS the client identity
  (SP9 semantics hold on every carrier);
- framing: the default carrier uses the §3 length-prefixed frame
  format on a raw byte stream. A carrier with native message
  framing (e.g. WebSocket messages, HTTP request/response bodies)
  MAY map one transport unit to one protocol frame instead of
  re-wrapping the §3 length prefix; the frame header fields
  (`msg_type`, `flags`, `correlation_id`) always travel with the
  frame.

A carrier MUST NOT inspect, transform, reorder, or act on message
bodies — the body is opaque bytes to the carrier.

> Note (non-normative). gRPC appears in this chapter's heritage
> notes as the protocol reovim moved away from; under SP16 it
> re-enters only as a *possible carrier* — an in-house transport
> that happens to speak gRPC framing could move protocol frames —
> never as the message protocol itself.

## 2. Transport

> **SP9 — One client is one stream connection; UDS primary, TCP
> secondary.** A client opens exactly one stream-oriented connection
> to the server. The connection IS the client identity (no separate
> create or login). The default and fastest transport is a Unix-domain
> stream socket at the path resolved from
> `editor.host.[transport].socket-path`. A TCP listener at
> `editor.host.[transport].listen-addr` is the opt-in remote transport
> (SEC2 auth applies). The protocol is byte-identical on both. *Class*:
> kernel-enforced (runtime).

The connection carries a single ordered, reliable byte stream in each
direction. Both directions multiplex request, response, and
notification frames distinguished only by message-type tag and
correlation id (§4, §5). There is no second connection, no side
channel.

## 3. Frame format

Every byte on the wire belongs to a frame. A frame is a fixed 16-byte
header followed by exactly `body_len` body bytes:

```
 offset  size  field            type   encoding
 0       4     body_len         u32    little-endian
 4       2     msg_type         u16    little-endian
 6       2     flags            u16    little-endian
 8       8     correlation_id   u64    little-endian
 16      N     body             bytes  N == body_len; codec per §6
```

> **SP10 — Frame header is the fixed 16-byte little-endian prefix.**
> Every frame begins with `(body_len: u32, msg_type: u16, flags: u16,
> correlation_id: u64)`, all little-endian, followed by exactly
> `body_len` body bytes. A reader frames the stream by reading 16
> bytes, then `body_len` bytes. `body_len` MUST NOT exceed
> `editor.host.[limits].wire-max-frame-bytes`; a larger declared length
> is a protocol violation and the connection is closed with a reject
> frame (§9). *Class*: kernel-enforced (runtime).

The header `#[repr(C)]` layout is registered in the 6.3 catalog as
`FrameHeader` with the scope note "crosses the wire boundary, not the
cdylib boundary" — 6.3 remains the single layout authority, and the
wire-boundary scope keeps it distinct from the ABI cdylib boundary.

**Flags (`u16` bitfield).** `bit 0` = `MORE` (the body is a fragment;
a subsequent frame with the same `(msg_type, correlation_id)` and
`MORE` clear completes it — reserved for bodies exceeding the frame
cap; v1 servers MAY reject oversized bodies instead of fragmenting).
`bit 1` = `COMPRESSED` (reserved; v1 sets it to 0 and rejects 1).
Bits `2..=15` are reserved and MUST be 0; a frame with an unknown flag
bit set in the request direction is rejected `ProtocolViolation`.

## 4. Correlation

> **SP11 — Requests carry a client-allocated correlation id;
> responses echo it; notifications use 0.** Every request frame
> (direction *req*, §7) carries a non-zero `correlation_id` allocated
> by the client. The response frame (direction *resp*) for that request
> echoes the same id. Notification frames (direction *notify*) carry
> `correlation_id == 0`. A client matches a response to its request by
> the echoed id. When the server originates a correlated sequence (none
> in v1; reserved for future server-initiated requests), it allocates
> ids under the T1 `correlation_alloc` lock tier (2.3 §4). *Class*:
> kernel-enforced (runtime).

Client correlation-id allocation is connection-local; the client owns
the namespace and MUST NOT reuse an id while its response is
outstanding. The id is opaque to the server beyond echo.

## 5. Directions and the notification stream

A frame's **direction** is a property of its message type, not a wire
field. Five directions:

| Direction | Origin | correlation_id | Tag range (§7) |
|---|---|---|---|
| *handshake* | both | 0 | `0x0001..=0x00FF` |
| *req* | client → server | non-zero | `0x0100..=0x01FF` |
| *resp* | server → client | echoes req | `0x0200..=0x02FF` |
| *notify* | server → client | `0` | `0x0300..=0x03FF` |
| *error* | server → client | echoes req, or 0 | `0xFF00..=0xFFFF` |

> **SP12 — After Attach the server pushes notify frames on the same
> connection.** Following a successful `Attach`, the server emits
> unsolicited *notify* frames (the `AttachEvent` family, §7.4) on the
> same connection, interleaved with responses to any in-flight
> requests. The client distinguishes them from responses by tag range
> and `correlation_id == 0`. This replaces server-streaming RPC: there
> is no separate stream object — the connection's server→client
> direction is the notification stream. *Class*: kernel-enforced
> (runtime).

> Heritage (non-normative). The gRPC form modelled this as
> `rpc Attach(...) returns (stream AttachEvent)` — a dedicated
> server-streaming RPC. The framed protocol folds it into the single
> connection: notify frames simply flow after the Attach response.

## 6. The deterministic byte codec

The protocol uses **one** serialization world, identical to the
deterministic byte encoding the 6.3 `*Wire` catalog implies (6.1 §5
names 6.3 the single layout authority; this chapter consumes it). No
schema-compiler, no reflection library, no self-describing tags. Each
codec primitive is named so every field in §7 maps to exactly one:

| Primitive | Wire form |
|---|---|
| `u8` / `u16` / `u32` / `u64` | fixed-width little-endian, native byte count |
| `i16` / `i32` / `i64` | fixed-width little-endian two's-complement |
| `bool` | one `u8`, `0` or `1`; any other value rejected `InvalidArgument` |
| `bytes` | `len: u32` little-endian, then `len` raw bytes |
| `str` | a `bytes` whose payload is UTF-8; validated at decode, `Utf8Invalid` on failure |
| `list<T>` | `count: u32` little-endian, then `count` encodings of `T` |
| `enum` | a discriminant at the cited 6.3 enum's repr width (`u8` unless the catalog says otherwise — `ErrorCode` is `i32`), little-endian, followed by the variant's fields (§6.1) |
| `carrier` | `header: [u8; 8]` raw, then a `bytes` (the `(header, content)` pair of 6.3 §5) |

> **SP13 — Every message is a hand-written struct with a byte-identical
> round-trip.** Each message type has an
> `encode(&self, out: &mut [u8]) -> Result<usize, ErrorCode>`
> (returns the byte count written) and a
> `decode(buf: &[u8]) -> Result<Self, ErrorCode>` pair in
> `uapi/protocol`, plus an exact sizing fn
> `encoded_size(&self) -> usize`. The caller provides the output
> buffer (SP15: the protocol never allocates); `out.len() <
> encoded_size()` fails `BufferTooSmall` with nothing written. A
> caller sizes the buffer by `encoded_size()` exactly, or by the
> SP10 `wire-max-frame-bytes` bound, which `encoded_size()` of any
> legal frame body never exceeds. Encoding is deterministic: the
> same value always produces the same bytes, and
> `decode(encode(x)) == x` for every legal value, byte-for-byte
> (CF4 golden fixtures, successor to the proto golden bytes). Fields
> are encoded in the order listed in the §7 body table, with no
> padding between them. *Class*: CI (golden) + runtime.

> **SP17 — Decoded form is a zero-copy borrowing view; encode refuses
> over-cap values.** Under SP15's no-allocation rule, `decode` builds
> no owned collections: a decoded message borrows the input buffer
> (`Msg<'a>` bound to the `&'a [u8]`). Fixed-width fields decode by
> value; every variable-length field (`bytes`, `str`, `list<T>`,
> `carrier` content) is a validated view over the input — a borrowed
> slice for `bytes`/`str`/carrier content, and a count-prefixed
> accessor/iterator view for `list<T>`. ALL structural validation
> (bounds, UTF-8, discriminants, counts) completes before `decode`
> returns `Ok`, so view traversal afterwards cannot fail. The SP13
> byte contract `decode(encode(x)) == x` is unchanged by the borrowed
> shape. Symmetrically on the encode side: a value whose
> `encoded_size()` would exceed the SP10 `wire-max-frame-bytes` bound
> is not a legal frame — `encode` (and `encoded_size`-driven sizing)
> fails it with `ErrorCode::ResourceExhausted` and writes nothing; an
> illegal over-cap frame is never produced. *Class*: API shape
> (compile-time) + runtime.

### 6.1 Tagged unions (sibling-discriminated)

A field of an `enum` type encodes as its discriminant followed by the
selected variant's fields (none for fieldless enums), mirroring the
6.3 enum-repr convention (6.3 §1): payload-carrying enums are
`#[repr(C, u8)]` with a `u8` discriminant; fieldless enums use their
primitive repr's width (`ErrorCode` is `#[repr(i32)]` → 4 bytes
little-endian). The discriminant values are those in the cited 6.3
enum (e.g. `ErrorCode` §9, `RawInputKind` for input payloads). An
unknown discriminant in the request direction is `InvalidArgument`; in
the notify direction it is skipped per §10.

### 6.2 Carrier payloads (cursor / position)

Cursor and position payloads cross the wire as the `carrier` primitive:
the 8-byte `CursorHeader` / `PositionHeader` (6.3 §5) followed by a
`bytes` content slice. They carry a `str domain_name` for stable
identity remap (CR9) when they appear inside a message. An empty
cursor set is a `list<carrier>` with `count == 0`, never a sentinel
blob (CR3 — absence is structural).

> Heritage (non-normative). The gRPC form base64-encoded the carrier
> content into a proto `bytes` field (`CursorBlob`/`PositionBlob`,
> `content_b64`). The framed protocol carries the raw bytes
> length-prefixed; the base64 hop is removed.

## 7. Message inventory

This table is the **canonical** message-type enumeration. Phase 1's
chapter↔struct cross-check (CF5) is built from it: every row MUST have
a matching hand-written struct in `uapi/protocol` with the same tag,
direction, and field set. Tags are never reused or repurposed (AB15,
§10).

Tag-range allocation:

- `0x0001..=0x00FF` — *handshake* / lifecycle.
- `0x0100..=0x01FF` — *req* (client → server).
- `0x0200..=0x02FF` — *resp* (server → client, correlated).
- `0x0300..=0x03FF` — *notify* (server → client, `AttachEvent` family).
- `0xFF00..=0xFFFF` — *error* (reject frames).

| Tag | Message | Dir | Body fields (name: type → primitive) |
|---|---|---|---|
| `0x0001` | `Hello` | handshake | `protocol_major: u16`→u16; `protocol_minor: u16`→u16; `caps: list<str>`→list/str; `domain_codecs: list<str>`→list/str |
| `0x0002` | `HelloAck` | handshake | `protocol_major: u16`→u16; `protocol_minor: u16`→u16; `granted_caps: list<str>`→list/str; `server_name: str`→str |
| `0x0100` | `Attach` | req | `session_name: str`→str; `auth: bytes`→bytes; `caps: list<str>`→list/str; `domain_codecs: list<str>`→list/str |
| `0x0101` | `Detach` | req | `reason: str`→str |
| `0x0102` | `SendInput` | req† | `client_id: u64`→u64; `buffer_id: u64`→u64; `window_id: u64`→u64; `inputs: list<RawInput>`→list/enum (6.3 §7) |
| `0x0103` | `SwitchSession` | req | `session_name: str`→str |
| `0x0104` | `DestroySession` | req | `session_name: str`→str; `force: bool`→bool |
| `0x0105` | `RenameSession` | req | `old_name: str`→str; `new_name: str`→str |
| `0x0106` | `DebugRead` | req | `op_kind: u32`→u32; `payload: bytes`→bytes; `subscribe: bool`→bool |
| `0x0107` | `DebugDrive` | req | `op_kind: u32`→u32; `payload: bytes`→bytes |
| `0x0108` | `PkgSync` | req | `targets: list<str>`→list/str; `dry_run: bool`→bool |
| `0x0109` | `PkgVerify` | req | `targets: list<str>`→list/str |
| `0x010A` | `ConfigDump` | req | `namespace: str`→str; `show_secrets: bool`→bool |
| `0x010B` | `ConfigValidate` | req | `toml: bytes`→bytes; `namespace: str`→str |
| `0x0200` | `AttachAck` | resp | `client_id: u64`→u64; `domain_table: list<DomainEntry>`→list/struct (§8) |
| `0x0201` | `DetachAck` | resp | (empty) |
| `0x0202` | `InputAck` | resp | `accepted: u64`→u64; `dropped: u64`→u64 |
| `0x0203` | `SwitchSessionAck` | resp | (empty — projection follows on notify) |
| `0x0204` | `DestroySessionAck` | resp | (empty) |
| `0x0205` | `RenameSessionAck` | resp | (empty) |
| `0x0206` | `DebugReadEvent` | resp | `op_kind: u32`→u32; `payload: bytes`→bytes; `final: bool`→bool |
| `0x0207` | `DebugDriveAck` | resp | `op_kind: u32`→u32; `result: enum ErrorCode`→enum; `payload: bytes`→bytes |
| `0x0208` | `PkgSyncProgress` | resp | `target: str`→str; `done: u32`→u32; `total: u32`→u32 |
| `0x0209` | `PkgSyncDone` | resp | `result: enum ErrorCode`→enum; `summary: str`→str |
| `0x020A` | `PkgVerifyAck` | resp | `result: enum ErrorCode`→enum; `report: bytes`→bytes |
| `0x020B` | `ConfigDumpResponse` | resp | `toml: bytes`→bytes |
| `0x020C` | `ConfigValidateResponse` | resp | `result: enum ErrorCode`→enum; `detail: str`→str |
| `0x0301` | `AttachEvent::Frame` | notify | `buffer_id: u64`→u64; `window_id: u64`→u64; `frame: bytes`→bytes |
| `0x0302` | `AttachEvent::Diff` | notify | `buffer_id: u64`→u64; `window_id: u64`→u64; `diff: bytes`→bytes |
| `0x0303` | `AttachEvent::Cursor` | notify | `buffer_id: u64`→u64; `window_id: u64`→u64; `cursors: list<carrier>`→list/carrier |
| `0x0304` | `AttachEvent::Projection` | notify | `buffer_id: u64`→u64; `projection: bytes`→bytes |
| `0x0305` | `AttachEvent::DomainTableDelta` | notify | `added: list<DomainEntry>`→list/struct; `removed: list<DomainEntry>`→list/struct |
| `0x0306` | `AttachEvent::SessionPivot` | notify | `session_name: str`→str |
| `0x0307` | `AttachEvent::SessionDestroyed` | notify | `session_name: str`→str |
| `0x0308` | `AttachEvent::ClientLeft` | notify | `client_id: u64`→u64; `reason: str`→str |
| `0x0309` | `AttachEvent::ServerDraining` | notify | `grace_ms: u32`→u32 |
| `0xFF00` | `Reject` | error | `code: enum ErrorCode`→enum (6.3 §2.3); `detail: str`→str |

† `SendInput` direction is *req* but uncorrelated on the hot path; see
§7.5.

`DomainEntry` (§8) and `RawInput` (6.3 §7, with its per-kind payloads)
are nested structs encoded inline by the codec; they are not
top-level message types.

### 7.4 The AttachEvent family

Tags `0x0301..=0x0309` are exactly the nine `AttachEvent` notify
types (`0x0300` is unallocated; a tag is allocated only with a
specified behaviour). After `AttachAck` (the response to `Attach`),
the server pushes these on the connection until detach or close. They
carry `correlation_id == 0`.

### 7.5 SendInput shape (pinned)

> **SendInput is an uncorrelated fire-and-forget hot-path frame with
> periodic correlated acks.** A `SendInput` frame (tag `0x0102`)
> carries `correlation_id == 0` on the hot path: the client does not
> wait for a per-batch response. The server applies the input and
> emits the resulting projection/cursor notify frames as the
> observable effect. The server emits an `InputAck` (tag `0x0202`)
> bearing a non-zero correlation id **only** when the client sets a
> non-zero correlation id on a `SendInput` frame to request a
> flow-control checkpoint (carrying cumulative `accepted` / `dropped`
> counts since the last ack). *Class*: kernel-enforced (runtime).

Rationale: the keystroke→render path must not block on a round-trip
ack. The visible response to input is the rendered frame, not an ack.
Correlated acks exist for backpressure accounting (§10) and for tests
that need a synchronisation point, on demand, not per batch.

> Heritage (non-normative). The gRPC form used a bidirectional
> streaming RPC `SendInput(stream InputBatch) returns (stream
> InputAck)`. The framed protocol keeps the input-batch shape but makes
> the ack opt-in rather than one-per-batch.

## 8. DomainTable

A `DomainEntry` is a nested struct (CR9):

| Field | Type | Primitive |
|---|---|---|
| `domain_name` | `str` | str (UTF-8, stable name) |
| `inner_id` | `u32` | u32 |
| `has_display` | `bool` | bool |
| `has_semantic` | `bool` | bool |

`AttachAck` carries the initial `list<DomainEntry>`;
`AttachEvent::DomainTableDelta` carries `added` / `removed` lists on
codec register/unregister. A client lacking a local codec for a
`(domain_name, inner_id)` follows the CR10 fallback. Entry count is
bounded by `editor.host.[limits].max-domain-table-entries` (CR17 —
the carrier-tier cap, not a transport rename).


## 9. Reject / error model

There is one error frame: `Reject` (tag `0xFF00`), body
`(code: ErrorCode, detail: str)`. `ErrorCode` is the 6.3 §2.3
`#[repr(i32)]` enum, encoded by the codec as its `i32` discriminant
(little-endian); `detail` is a human-readable UTF-8 string. A `Reject`
echoes the offending request's `correlation_id` when one exists,
else 0. An out-of-range `code` discriminant on `Reject` decode maps
to `ErrorCode::Generic` rather than rejecting the frame (a `Reject`
is already the error path; mirroring AB14's out-of-range slot-return
rule keeps one degraded-code convention).

> **SP14 — Failures are a single reject frame carrying an ErrorCode.**
> Every request failure, capability rejection, protocol violation, and
> handshake refusal is reported as a `Reject` frame (tag `0xFF00`)
> carrying a 6.3 `ErrorCode` and a detail string. There is no
> per-message bespoke error type. A `Reject` with
> `code == ProtocolViolation` is terminal: the server closes the
> connection after sending it. *Class*: kernel-enforced (runtime).

Failure-condition mapping to the one 6.3 `ErrorCode` vocabulary (AB14):

| Failure condition | Framed `ErrorCode` |
|---|---|
| Second Attach on one connection | `Conflict` |
| Server shutting down | `Busy` |
| Malformed request argument | `InvalidArgument` |
| Unknown channel / target | `NotFound` |
| Missing required capability | `PermissionDenied` |
| Clean stream end | no frame; connection closes cleanly |
| Frame/codec protocol error | `ProtocolViolation` (terminal) |

> Heritage (non-normative). Earlier drafts used per-RPC gRPC status codes for
> these conditions (`FAILED_PRECONDITION`, `UNAVAILABLE`,
> `INVALID_ARGUMENT`, `NOT_FOUND`, `PERMISSION_DENIED`, `OK`-close).
> The framed protocol collapses them onto the one 6.3 `ErrorCode`
> vocabulary above.

## 10. Versioning, handshake, unknown-tag policy, backpressure

### 10.1 Handshake

The connection opens with a `Hello` frame (tag `0x0001`,
`correlation_id == 0`) carrying `protocol_major`, `protocol_minor`,
and the client's capability and codec lists. The server replies
`HelloAck` (tag `0x0002`) with its own `protocol_major`/`minor`, the
granted capability set, and `server_name`; or it replies `Reject`
(`code == IncompatibleApi` on major mismatch, `code == ProtocolViolation`
on a malformed `Hello`) and closes. No other frame may precede the
`Hello`/`HelloAck` exchange.

> **SP9.1 (handshake) — Connection opens with Hello / HelloAck.** The
> first client frame MUST be `Hello`; the first server frame MUST be
> `HelloAck` or `Reject`. A server receiving any other first frame
> closes with `ProtocolViolation`. *Class*: kernel-enforced (runtime).

### 10.2 Versioning

- **Major bump** = a parallel protocol served alongside the shipped
  one. The server accepts a `Hello` for any major it still serves and
  answers on that major's frame layout; shipped majors are never
  removed (AB15, eternal-wire rule restated for the framed wire).
- **Minor bump** = additive: new message-type tags and/or trailing
  body fields appended to existing messages. A decoder MUST ignore
  body bytes beyond the fields it knows (trailing-bytes-tolerant
  decode), so an old peer reads a new minor's message correctly up to
  its known fields.

### 10.3 Unknown-tag policy

> **Unknown message-type tags: skip in notify, reject in req.** A
> *notify* frame (`0x0300..=0x03FF`) with a tag the client does not
> recognise is **skipped** — the client reads `body_len` bytes and
> discards them, staying framed (this is how a new minor's notify type
> is forward-compatible). A *req* frame with a tag the server does not
> recognise is **rejected** `ProtocolViolation` (terminal), because a
> request the server cannot interpret cannot be silently dropped
> without desynchronising the client's correlation state. *Class*:
> kernel-enforced (runtime).

### 10.4 Backpressure

Per-connection window flow control governs the server→client notify
stream and client→server input. Limits live under
`editor.host.[limits].wire-*` (transport-neutral keys; see the
heritage note below). The server applies CR-style flow control on
notify-frame output (the `wire-recv-window-bytes` window); the client
input side is
capped per `wire-input-rate` (CR15-style). When the notify window is
exhausted the server coalesces per the channel's drop policy (DS4
classes apply: `state` newest-wins, `frame` drops intermediates).
The `wire-max-frame-bytes` cap is enforced at the codec boundary in
both directions: `encode` refuses an over-cap value with
`ResourceExhausted` (SP17), and a received header whose `body_len`
exceeds the cap is rejected before body decode.

> Heritage (non-normative). The window keys were `grpc-*` under the
> gRPC form; they are renamed `wire-*` (transport-neutral) with
> identical semantics.

## 11. Carried rules (SP1..SP8)

The rule bodies, restated in framed-protocol vocabulary. Semantics are
unchanged from the prior wire form; only the mechanism words change
(request-response call → request message type; server-stream → notify
frames; status code → `Reject` frame `ErrorCode`).

> **SP1 — Single-stream-per-client.** The server rejects a second
> `Attach` on the same connection with a `Reject` frame carrying
> `ErrorCode::Conflict`; the first attach is unaffected. *Class*:
> kernel-enforced (runtime).

> **SP2 — `SwitchSession` pivots in place.** The server answers
> `SwitchSessionAck`, then emits `AttachEvent::SessionPivot` followed
> by the new session's initial-projection notify sequence on the same
> connection. No reconnect. *Class*: kernel-enforced (runtime).

> **SP3 — Graceful connection close is implicit detach.** When the
> client closes its connection cleanly, the server reaps the client —
> focus chains release through the LF12 teardown ordering — and emits
> `AttachEvent::ClientLeft { reason: "connection-closed" }` to
> remaining clients. An explicit `Detach` request exists for
> telemetry / DS6-recording reasons. *Class*: kernel-enforced
> (runtime).

> **SP4 — `DestroySession` is broadcast.** Every attached client
> receives `AttachEvent::SessionDestroyed`; their connections are not
> error-closed by the destroy — the notify is a clean event, and each
> client detaches or switches in response. *Class*: kernel-enforced
> (runtime).

> **SP5 — `RenameSession` migrates persisted state eagerly.** The
> rename updates both the user-visible name and `SessionId(Arc<str>)`.
> Persisted state keyed by the old name (2.4 §2,
> `state/sessions/<session-id>/`) migrates as part of the rename, not
> lazily. *Class*: kernel-enforced (runtime).

> **SP6 — No `CreateClient` message.** Client identity is the
> connection (SP9); the `Hello`/`Attach` exchange registers the
> client. Capability declaration rides `Hello` and `Attach` (§10.1,
> §7). There is no separate create message in the inventory (§7).
> *Class*: spec-asserted.

> **SP7 — No `KickClient` message in v1.** `DestroySession` with
> `force == true` is the only authorised tear-down path; per-client
> admin operations are a follow-on protocol minor. The inventory (§7)
> contains no kick message. *Class*: spec-asserted.

> **SP8 — `ServerDraining` precedes shutdown detach.** At LF15 phase 1
> (2.2 §9), the server sends `AttachEvent::ServerDraining { grace_ms }`
> on every attached client's connection. Clients SHOULD detach
> voluntarily within the grace window; phase 2 closes the remainder.
> Connections closed by shutdown close cleanly (no `Reject`); a new
> `Attach` after shutdown begins is rejected with a `Reject` frame
> carrying `ErrorCode::Busy` (LF15 phase 0). The message is additive on
> the notify stream — a protocol minor. *Class*: runtime.

**Reshape note (SP1..SP8).** Semantics identical to the prior form.
The message inventory itself is contract surface, not a numbered rule:
adding, renaming, or removing a message type is a protocol major —
which per §10.2 and AB15 means a new major served alongside, never an
edit to a shipped tag.

## 12. Multi-client attach ordering (resolved)

> **Per-session notify frames are emitted in turn-gate order; two
> clients attached to one session observe the same projection
> sequence.** Attach and detach against a session serialise on the
> `sessions` map tier (T12, 2.3 §4) and the innermost `Session.state`
> tier (T13); dispatch that produces projections serialises through the
> session `turn_gate` (2.3 §4, orthogonal to the tier list). Because
> every state-mutating dispatch passes the turn gate before emitting,
> the order of `AttachEvent::Projection` / `Cursor` / `Diff` frames is
> the turn-gate order, and that single order is fanned out to every
> client attached to the session. Two clients therefore observe the
> same projection sequence for the same session; per-connection
> backpressure (§10.4) may coalesce intermediate frames independently
> per client, but never reorders them. *Class*: kernel-enforced
> (runtime).

## Open items

1. ~~Full message inventory per request type~~ — resolved (#782 §7):
   every message type enumerated with tag, direction, and body fields;
   the §7 table is the canonical CF5 source.
2. ~~Provenance~~ — resolved (#782): the wire form is owned by the
   hand-written message structs in `uapi/protocol` plus the §6 codec
   spec; CF5 cross-checks chapter ↔ struct (1.4, 9.1).
3. ~~Multi-client attach to one session — locking and ordering~~ —
   resolved (#782 §12): turn-gate-ordered notify, T12/T13 serialise
   attach/detach, shared projection sequence.
4. ~~Streaming-input vs request/response for `SendInput`~~ — resolved
   (#782 §7.5): uncorrelated fire-and-forget hot-path frame with
   on-demand correlated flow-control acks.

## A worked frame (golden fixture)

A complete `Hello` frame: `protocol_major = 1`, `protocol_minor = 0`,
`caps = ["render.cells"]`, `domain_codecs = ["text.utf8"]`, sent as the
first frame (`correlation_id = 0`, `flags = 0`, `msg_type = 0x0001`).

**Body** (codec per §6), built field by field:

```
protocol_major: u16 = 1        → 01 00
protocol_minor: u16 = 0        → 00 00
caps: list<str>, count = 1     → 01 00 00 00
  "render.cells"  (12 bytes)   → len 0C 00 00 00, then bytes:
                                  72 65 6E 64 65 72 2E 63 65 6C 6C 73
domain_codecs: list<str>, count = 1 → 01 00 00 00
  "text.utf8"  (9 bytes)       → len 09 00 00 00, then bytes:
                                  74 65 78 74 2E 75 74 66 38
```

Body length = 2 + 2 + 4 + (4 + 12) + 4 + (4 + 9) = **41 bytes**
(`0x29`).

**Header** (§3): `body_len = 41 = 0x00000029` → `29 00 00 00`;
`msg_type = 0x0001` → `01 00`; `flags = 0` → `00 00`;
`correlation_id = 0` → `00 00 00 00 00 00 00 00`.

**Full frame** (16-byte header + 41-byte body = 57 bytes), annotated:

```
offset  bytes                              meaning
0x00    29 00 00 00                        body_len = 41
0x04    01 00                              msg_type = 0x0001 (Hello)
0x06    00 00                              flags = 0
0x08    00 00 00 00 00 00 00 00            correlation_id = 0
0x10    01 00                              protocol_major = 1
0x12    00 00                              protocol_minor = 0
0x14    01 00 00 00                        caps.count = 1
0x18    0C 00 00 00                        caps[0].len = 12
0x1C    72 65 6E 64 65 72 2E 63 65 6C 6C 73   "render.cells"
0x28    01 00 00 00                        domain_codecs.count = 1
0x2C    09 00 00 00                        domain_codecs[0].len = 9
0x30    74 65 78 74 2E 75 74 66 38         "text.utf8"
                                           (frame ends at 0x39 = 57)
```

Flat hex (57 bytes):

```
29 00 00 00 01 00 00 00 00 00 00 00 00 00 00 00
01 00 00 00 01 00 00 00 0C 00 00 00 72 65 6E 64
65 72 2E 63 65 6C 6C 73 01 00 00 00 09 00 00 00
74 65 78 74 2E 75 74 66 38
```

A reimplementer's decoder MUST produce exactly the field values above
from these bytes, and its encoder MUST produce exactly these bytes
from those values (SP13).

## Conformance

| Rule | Fixture |
|---|---|
| SP1 | Second `Attach` on one connection → `Reject{Conflict}`; first attach's notify stream unaffected. |
| SP2 | `SwitchSession` mid-stream → `SwitchSessionAck` then `AttachEvent::SessionPivot` then full initial projection; no transport reconnect observed. |
| SP3 | Close client connection gracefully → server emits `AttachEvent::ClientLeft{connection-closed}`; focus chains released. |
| SP4 | `DestroySession` with two clients attached → both receive `AttachEvent::SessionDestroyed`; neither connection error-closes. |
| SP5 | Rename session, restart server → state restores under the new name; old-name dir gone. |
| SP6 | Fresh connection's `Hello`/`Attach` registers the client; inventory (§7) contains no create message (CF5 check). |
| SP7 | Inventory (§7) contains no kick message (CF5 check); `force=true` destroy tears down an unresponsive client. |
| SP8 | Shutdown with an attached client → `AttachEvent::ServerDraining` observed before clean close; `Attach` after shutdown begins → `Reject{Busy}`. |
| SP9 | Open one UDS connection, send a second `Attach` → `Reject{Conflict}`; TCP path behaves identically. |
| SP9.1 | First client frame is not `Hello` → `Reject{ProtocolViolation}` and close. |
| SP10 | Feed a frame whose `body_len` exceeds `wire-max-frame-bytes` → `Reject{ProtocolViolation}` and close; framer stays byte-aligned on legal frames. |
| SP11 | Request with correlation id `N` → response echoes `N`; concurrent requests with distinct ids each get the matching response. |
| SP12 | After `AttachAck`, server pushes `AttachEvent::*` notify frames with `correlation_id == 0` on the same connection. |
| SP13 | Codec round-trip golden: `decode(encode(x)) == x` byte-for-byte for every message type; the §"worked frame" `Hello` bytes match exactly (CF4 successor). |
| SP14 | Capability rejection / protocol violation → single `Reject` frame with the mapped `ErrorCode`; `ProtocolViolation` closes the connection. |
| (handshake) | `Hello` with an unserved major → `Reject{IncompatibleApi}`. |
| (unknown tag) | Notify frame with an unknown tag → client skips `body_len` bytes, stays framed; req frame with unknown tag → `Reject{ProtocolViolation}`. |
| (backpressure) | Slow client → notify window exhausts → frames coalesce per DS4 drop policy; no reorder; correlated `InputAck` reports `dropped: N`. |
| (inventory cross-check, CF5) | Every §7 row has a matching `uapi/protocol` struct with the same tag, direction, and field set; a struct without a §7 row, or a row without a struct, fails CI. |
| (attach flow) | Client without required caps → `Reject{PermissionDenied}` at `Attach`. |
| (DomainTable delta) | Codec registers post-attach → server emits `AttachEvent::DomainTableDelta`. |
| (CR10 hop) | Client sees a Domain in the table without a local codec → fallback path. |
