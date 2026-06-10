# 8.4 — Client Debug

**Scope.** Client-side debug capability: how a client cdylib
exposes a sibling debug vtable that the server routes to.

**Heritage.** v4 README §13.2 / §15; CL4 / CL5.

**Locked rules.** Carried per CL4 / CL5 in 8.1.

---

## 1. The capability

Client debug is a **separate capability** from rendering. A render
driver does not double as the debug surface (CL4).

```c
#[repr(C)]
pub struct ClientDebugVtable {
    pub header: VtableHeader,                   // kind = CapabilityClient

    pub drive_input: unsafe extern "C" fn(
        ctx: *mut c_void,
        client: ClientId, buffer: BufferId, window: WindowId,
        input: RawInput,
        err: *mut ErrorBuf,
    ) -> ErrorCode,

    pub capture_frame: unsafe extern "C" fn(
        ctx: *mut c_void,
        client: ClientId, buffer: BufferId, window: WindowId,
        out: *mut FrameBlob,
        err: *mut ErrorBuf,
    ) -> ErrorCode,

    pub capture_projection_diff: unsafe extern "C" fn(
        ctx: *mut c_void,
        client: ClientId, buffer: BufferId, window: WindowId,
        out: *mut DiffBlob,
        err: *mut ErrorBuf,
    ) -> ErrorCode,

    pub subscribe_events: unsafe extern "C" fn(
        ctx: *mut c_void,
        filter: ByteSlice,
        out: *mut SubscriberHandle,
        err: *mut ErrorBuf,
    ) -> ErrorCode,
}
```

Symbol: `REOVIM_CAPABILITY_DEBUG_VTABLE`.

## 2. Routing

The server-side debug surface (7.2) routes opaque payloads through
the gRPC protocol (7.3). The client capability decodes; the server
does not (CL5). For example:

```
client gRPC stub
  → server.DebugDrive(stream)
    → server forwards opaque payload to client capability vtable
      → capability decodes the op kind, runs drive_input
```

The server has no compile-time knowledge of `op_kind` mapping to
specific capability calls; the mapping lives in the capability.

## 3. Capability negotiation

A client declares the debug capabilities at attach (per 7.3 §7;
vocabulary fixed in 7.2 §5.1):

```proto
caps = ["render.cells", "debug.read", "debug.mutate"]
```

A client without `debug.read` cannot subscribe to debug events or
capture frames; one without `debug.mutate` cannot call drive
operations (DS13).

## 4. Mutate audit

Per DS13 (7.2 §3), every drive operation emits a
`debug.drive.start|ok|fail` audit event with a correlation ID, the
client identity, and the op kind.

## 5. Lifecycle

Standard cdylib lifecycle. Capability cdylibs can be unloaded;
debug subscriptions are revoked on unload.

## Open items

1. Capability vocabulary allocation — `debug` is one capability;
   future `debug.replay`, `debug.fuzz` may split. Default: keep as
   one capability with op-kind discriminator.
2. Whether debug capability can be packaged separately from render
   driver (yes; 8.3 supports multi-vtable).
3. Replay capability (record + replay debug streams) — out of v4
   target.

## Conformance

This chapter is a sketch. Conformance fixtures arrive once
debug-capability cdylib lands; ties into DS13.
