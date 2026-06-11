# 6.3 — Type Catalog

**Scope.** The single source of truth for every `#[repr(C)]` layout
that crosses the ABI boundary. Other chapters cite this catalog;
this chapter defines.

**Locked rules.** None directly; this chapter binds AB3 / AB12 /
CFG6 / CR1..CR2 / SVC2 layouts.

---

## 1. Catalog discipline

- This chapter holds the only authoritative `#[repr(C)]` definitions.
- Other chapters reference by type name; they do not redefine.
- `uapi/<crate>/src/*.rs` mirrors this catalog; mismatch fails CI
  (golden offset/size tests).
- Generated C header is built from this catalog (generator TBD).
- **Enum repr convention** `(resolved #782)`: fieldless enums use a
  primitive-only repr (`#[repr(i32)]`, `#[repr(u8)]`) — rustc rejects
  `#[repr(C, int)]` on fieldless enums (E0566), and the primitive
  repr fixes the identical discriminant layout. Payload-carrying
  enums use `#[repr(C, u8)]` (tagged-union layout).

If a layout is sketched in another chapter, a note `(catalog: §N)`
points back here.

## 2. Foundational types

### 2.1 Versions and identifiers

```rust
#[repr(C)]
pub struct AbiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub pad:  u16,
}

#[repr(C)]
pub struct Version {     // ApiVersion
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub pad:  u16,
}

#[repr(transparent)]
pub struct CdylibId(pub NonZeroU32);

#[repr(transparent)]
pub struct DomainId(pub NonZeroU32);

#[repr(transparent)]
pub struct ClientId(pub usize);

#[repr(transparent)]
pub struct SessionId(pub Arc<str>);          // not crossable as-is; opaque handle when crossing

#[repr(transparent)]
pub struct BufferId(pub usize);

#[repr(transparent)]
pub struct WindowId(pub usize);

#[repr(transparent)]
pub struct DomainAttachmentId(pub u64);

#[repr(transparent)]
pub struct PendingAttachmentId(pub u64);

#[repr(transparent)]
pub struct UndoGroupId(pub u64);

#[repr(transparent)]
pub struct ServiceKey(pub NonZeroU32);
```

`SessionId(Arc<str>)` does not cross directly; HostApi exchanges an
opaque `*mut c_void` session handle.

### 2.2 Byte-slice types

```rust
#[repr(C)]
pub struct ByteSlice {                       // immutable, caller-owned-for-call
    pub data: *const u8,
    pub len:  usize,
}

#[repr(C)]
pub struct ByteSliceMut {                    // mutable, caller-owned-for-call
    pub data: *mut u8,
    pub len:  usize,
}

#[repr(C)]
pub struct StrSlice {                        // ByteSlice with UTF-8 invariant
    pub data: *const u8,
    pub len:  usize,
}

#[repr(C)]
pub struct ByteBuf {                         // callee-owned-until-release
    pub data: *mut u8,
    pub cap:  usize,
    pub len:  usize,
    pub free: unsafe extern "C" fn(*mut u8, usize),
}

#[repr(C)]
pub struct ErrorBuf {
    pub data: *mut u8,
    pub cap:  usize,
    pub len:  usize,
}
```

### 2.3 Result type

```rust
#[repr(i32)]
pub enum ErrorCode {
    Ok                          = 0,
    Generic                     = 1,
    IncompatibleAbi             = 2,
    IncompatibleApi             = 3,
    NotFound                    = 4,
    Conflict                    = 5,
    InvalidArgument             = 6,
    ResourceExhausted           = 7,
    ProtocolViolation           = 8,
    Busy                        = 9,
    Stale                       = 10,
    PermissionDenied            = 11,
    Panic                       = 12,
    Cancelled                   = 13,
    Timeout                     = 14,
    SchemaInvalid               = 15,
    NamespaceConflict           = 16,
    IllegalTrustClass           = 17,
    ConfigSliceTooLarge         = 18,
    Utf8Invalid                 = 19,
    IllegalProjectHostSection   = 20,
    IllegalKind                 = 21,
    ShortVtable                 = 22,
    CodecGone                   = 23,
    NotActive                   = 24,
    RollbackFailed              = 25,
    BufferTooSmall              = 26,   // caller-provided buffer under encoded_size() (7.3 SP13)
    // future codes appended; values >= 240 reserved
}
```

### 2.4 Log level

Used by `hostapi_log_emit` (9.5 §7) and the DS12 event shape.

```rust
#[repr(u8)]
pub enum LogLevel {
    Unknown = 0,   // reserved-invalid; rejected at emission
    Trace   = 1,
    Debug   = 2,
    Info    = 3,
    Warn    = 4,
    Error   = 5,
}
```

## 3. Vtable header

```rust
#[repr(C)]
pub struct VtableHeader {
    pub abi:          AbiVersion,
    pub api:          Version,
    pub size_of_self: usize,
    pub kind:         ManifestKind,
    pub flags:        u32,
}

#[repr(u8)]
pub enum ManifestKind {
    Unknown          = 0,   // reserved-invalid; rejected at load (6.2 §7)
    ModuleServer     = 1,
    DriverServer     = 2,
    ModuleClient     = 3,
    DriverClient     = 4,
    CapabilityClient = 5,
    DomainServer     = 6,
    ProviderServer   = 7,
    StreamScheme     = 8,
}
```

The set is final for ABI major 1; sub-kinds discriminate via
manifest `[[vtable]]` kind strings, not enum growth (6.2 §7).

## 4. Config types

(See 6.4 for the full ConfigSlice ABI.)

```rust
#[repr(C)]
pub struct ConfigSlice {
    pub abi_version:    u32,
    pub namespace:      StrSlice,
    pub schema_version: u32,
    pub entries:        *const ConfigKvp,
    pub len:            usize,
    pub total_bytes:    usize,
    pub reserved:      u32,
}

#[repr(C)]
pub struct ConfigKvp {
    pub dotted_key: StrSlice,
    pub kind:       ConfigValueKind,
    pub value:      ConfigValue,
    pub source:     ConfigSource,
    pub flags:      u32,
}

#[repr(u8)]
pub enum ConfigValueKind {
    Unknown       = 0,
    Bool          = 1,
    U32           = 2,
    I32           = 3,
    U64           = 4,
    I64           = 5,
    F64           = 6,
    String        = 7,
    Path          = 8,
    Enum          = 9,
    CanonicalToml = 10,
}

#[repr(u8)]
pub enum ConfigSource {
    Default = 0,
    System  = 1,
    User    = 2,
    Project = 3,
    Env     = 4,
    Cli     = 5,
    Force   = 6,
}

#[repr(C)]
pub union ConfigValue {
    pub bool_value: u8,
    pub u32_value:  u32,
    pub i32_value:  i32,
    pub u64_value:  u64,
    pub i64_value:  i64,
    pub f64_value:  f64,
    pub bytes:      ByteSlice,
}
```

## 5. Coordination carriers

```rust
#[repr(C)]
pub struct PositionHeader { pub bytes: [u8; 8] }

#[repr(C)]
pub struct CursorHeader   { pub bytes: [u8; 8] }
```

(Header byte-layout per CR2.)

`PositionCarrier` and `CursorCarrier` cross the boundary as
`(header: 8 bytes, content: ByteSlice)`. The spec-level Rust
struct holds `Vec<u8>`; the wire-level pair is laid out as:

```rust
#[repr(C)]
pub struct PositionCarrierWire {
    pub header:  PositionHeader,
    pub content: ByteSlice,
}

#[repr(C)]
pub struct CursorCarrierWire {
    pub header:  CursorHeader,
    pub content: ByteSlice,
}
```

## 6. Service descriptor

```rust
#[repr(C)]
pub struct ServiceDescriptor {
    pub key:              ServiceKey,
    pub owner_cdylib_id:  CdylibId,
    pub abi:              AbiVersion,
    pub api:              Version,
    pub type_name:        StrSlice,
    pub vtable_ptr:       *const c_void,
    pub vtable_size:      usize,
    pub handle:           *mut c_void,
    pub flags:            u32,           // SEND_SAFE | SYNC_SAFE | HOSTAPI_REENTRANT | DROP_MAY_CALL_HOSTAPI
    pub drop_fn:          unsafe extern "C" fn(*mut c_void),
}
```

## 7. RawInput

```rust
#[repr(C)]
pub struct RawInput {
    pub kind:    RawInputKind,
    pub payload: ByteSlice,
}

#[repr(u8)]
pub enum RawInputKind {
    Key     = 1,
    Mouse   = 2,
    Text    = 3,
    Paste   = 4,
    Web     = 5,    // reserved; payload locked with the WASM-peer decision (out of target)
    Trigger = 6,
    Ime     = 7,
    // 128..=255 reserved for module-defined custom kinds (per-cdylib; see 8.3 CL10)
}
```

### 7.1 `Key` payload

```rust
#[repr(C)]
pub struct KeyEvent {
    pub keycode:      u32,      // §7.1.1 vocabulary
    pub mods:         u8,       // §7.1.2 bitfield
    pub action:       u8,       // 0 = press, 1 = release, 2 = repeat
    pub pad:         [u8; 2],
    pub timestamp_ns: u64,      // monotonic
    pub utf8:         [u8; 8],  // utf8[0] = len (0..=7); utf8[1..=len] = bytes
}
```

Size 24, align 8. Platforms that cannot observe key release
(classic terminals) emit `press` only; modules MUST NOT require
release events for correctness.

**7.1.1 Keycode vocabulary.** Values `0x0000_0001..=0x0000_FFFF`
are USB HID usage IDs from usage page `0x07` (Keyboard/Keypad) —
an industry vocabulary stable since 1996 and platform-portable.
Values `0x0001_0000..` are the Reovim extension range for
software-only keys (`Compose = 0x0001_0001`, `ImeToggle =
0x0001_0002`; range grows additively on api-minor). Input drivers
translate platform events (raw terminal escape decoding, DOM `code`, etc.) to this
vocabulary.

**7.1.2 Modifier bitfield.** `bit 0` Shift, `1` Ctrl, `2` Alt,
`3` Super, `4` AltGr, `5` CapsLock, `6` NumLock, `7` reserved.

**7.1.3 Inline UTF-8.** The translated character for the
keypress, when the driver knows it (max 7 bytes covers any single
scalar). Text *runs* travel as `Text`, not as per-key events.

### 7.2 `Mouse` payload

```rust
#[repr(C)]
pub struct MouseEvent {
    pub action:       u8,       // 0 = press, 1 = release, 2 = motion, 3 = scroll
    pub button:       u8,       // 0 = none, 1 = left, 2 = right, 3 = middle, 4..=7 = extra
    pub mods:         u8,       // same bitfield as KeyEvent
    pub flags:        u8,       // bit 0: pixel fields valid
    pub col:          i32,      // grid cell column, window-local (primary frame)
    pub row:          i32,      // grid cell row, window-local
    pub px_x:         i32,      // pixel offset, window-local; valid iff flags bit 0
    pub px_y:         i32,
    pub scroll_x:     i16,      // scroll delta in 1/8-line units (smooth scroll)
    pub scroll_y:     i16,
    pub timestamp_ns: u64,
}
```

Size 32, align 8. Grid cells are the primary coordinate frame —
every platform can produce them; pixel fields are the optional
sub-cell refinement for platforms that have them (web, GUI).
Multi-touch is out of target; it arrives as a new kind, not
as `Mouse` growth.

### 7.3 `Text` / `Paste` payloads

The payload **is** the UTF-8 bytes — no header struct. Kernel
validates UTF-8 at ingress and bounds by
`kernel.host.[limits].max-text-input-bytes`. `Text` is a typed
run (bracketed input); `Paste` is an explicit clipboard paste —
same shape, distinct kind so policy modules can treat paste
specially (e.g. no keymap interpretation).

### 7.4 `Trigger` payload

```rust
#[repr(C)]
pub struct TriggerEvent {
    pub trigger_id:   u32,
    pub pad:         u32,
    pub timestamp_ns: u64,
}
```

Size 16, align 8. Opaque to the kernel beyond routing (5.2 §6).

### 7.5 `Ime` payload

```rust
#[repr(C)]
pub struct ImeEvent {
    pub phase:        u8,       // 0 = begin, 1 = update, 2 = commit, 3 = cancel
    pub pad:         [u8; 3],
    pub caret_byte:   u32,      // caret position within the preedit text, bytes
    pub timestamp_ns: u64,
    // payload continues: UTF-8 preedit (begin/update) or committed text (commit)
}
```

The payload is this 16-byte header followed by the UTF-8 text;
text length = `payload.len - 16`. Composition (CJK input, dead
keys) is first-class — an editor that treats IME as an
afterthought is not a 50-year editor. Keymap modules MUST pass
`Ime` events through to the focused Domain rather than
interpreting them as key sequences.

### 7.6 `Custom(128..=255)` payloads

Per-cdylib allocation; no global registry. The cdylib that emits a
custom kind MUST be the only consumer of it; the kernel routes by
the registered trigger/handler row and never interprets the
payload. A custom kind's payload schema is declared in the
cdylib's manifest for debug-surface decoding (8.3 CL10).

### 7.7 CLI / debug JSON boundary

The hot path carries only the `#[repr(C)]` records above. The
CLI / debug translation layer owns a JSON Schema per kind for
replay tooling and introspection; encode/decode happens at that
boundary only (5.2 §3). JSON schemas are golden-tested per CF4.


## 8. Stream substrate

```rust
#[repr(transparent)]
pub struct StreamId(pub u64);

#[repr(C)]
pub struct StreamHandleInfo {
    pub id:           StreamId,
    pub state:        StreamState,
    pub bytes_in_flight: u64,
    pub bytes_buffered:  u64,
}

#[repr(u8)]
pub enum StreamState {
    Init                 = 0,
    Running              = 1,
    BackpressureBlocked  = 2,
    Stale                = 3,
    Draining             = 4,
    Closed               = 5,
}
```

## 9. Domain tree

```rust
#[repr(C, u8)]
pub enum FocusEntryWire {
    Pending(PendingAttachmentId),
    Resolved(DomainAttachmentId),
}

#[repr(C, u8)]
pub enum FocusEntrySnapshot {
    Pending  { id: PendingAttachmentId,  domain_id: DomainId },
    Resolved { id: DomainAttachmentId,   domain_id: DomainId },
}

#[repr(C)]
pub struct FocusTransition {
    pub session_id_handle: *const c_void,    // opaque session handle (SessionId is Arc<str>)
    pub client_id:         ClientId,
    pub buffer_id:         BufferId,
    pub window_id:         WindowId,
    pub seq:               u64,              // monotonic per session
    pub before:            *const FocusEntrySnapshot,
    pub before_len:        usize,
    pub after:             *const FocusEntrySnapshot,
    pub after_len:         usize,
}

#[repr(C)]
pub struct DomainScopeWire {
    pub start: PositionCarrierWire,
    pub end:   PositionCarrierWire,
    pub flags: u8,
    pub pad:  [u8; 7],
}
```

`FocusTransition` (Q-4) carries the paired before/after snapshots
of the focus chain for one transition. Lifetime follows the same
init-call pattern as `ConfigSlice` (kernel-owned during the
observer callback; observer copies what it retains).

## 10. Manifest TOML schemas (cited)

Manifest TOML is parsed at runtime, not part of the binary ABI.
Listed here for traceability only; bodies are in:

- module manifest → `07-Surfaces/01-Package-Manager.md` §3,
- driver manifest → `07-Surfaces/01-Package-Manager.md` §4,
- capability manifest → `08-Client/05-Packaging.md` §2,
- stream-scheme manifest → `04-Domain-Substrate/04-Stream.md` §3.

## 11. Layout-stability rules

- A layout MUST NOT change without a major ABI bump.
- Adding a field to a `#[repr(C)]` struct is forbidden mid-major.
  Append-only behaviour belongs in vtables (AB3), not data
  structs.
- Adding a variant to a catalog enum — fieldless primitive-repr or
  payload-carrying `#[repr(C, u8)]` alike (§1) — is allowed across
  minor as long as old code receiving it returns
  `ErrorCode::IncompatibleApi`. Future-reserved values are at the
  high end (e.g. `ErrorCode` codes ≥ 240).
- Reordering fields is forbidden.
- Removing a field is a major break.
- **Post-v1.0, a major bump does not license removal** (AB15):
  a new major is added alongside shipped majors; every layout in
  this catalog that ever shipped stable remains supported by the
  loader forever. Golden-test artefacts for shipped majors are
  permanent.
- The wire frame header (§12) is governed by the protocol versioning
  rule (7.3 §10.2), not by this section's ABI-major rule.

## 12. Wire frame header

**Scope note — this layout crosses the *wire* boundary, not the
*cdylib* boundary.** Unlike every other type in this catalog, the
frame header is never passed across an `extern "C"` cdylib call; it
is the fixed prefix of every frame on the server-client stream
socket (7.3 §3). It is registered here so the catalog remains the
single layout authority — but its golden test is a wire-bytes
fixture, not a per-target-triple offset test, because it is
serialized field-by-field little-endian (it is never `transmute`d
from this struct). The struct exists so encoder/decoder code shares
one definition.

```rust
#[repr(C)]
pub struct FrameHeader {
    pub body_len:       u32,   // little-endian on the wire
    pub msg_type:       u16,   // little-endian on the wire
    pub flags:          u16,   // little-endian on the wire
    pub correlation_id: u64,   // little-endian on the wire
}
```

| offset | size | field | type | wire encoding |
|---|---|---|---|---|
| 0 | 4 | `body_len` | u32 | little-endian |
| 4 | 2 | `msg_type` | u16 | little-endian |
| 6 | 2 | `flags` | u16 | little-endian |
| 8 | 8 | `correlation_id` | u64 | little-endian |

Size 16, align 8 (natural). The on-wire form is the four fields in
declared order, each little-endian; total 16 bytes. 7.3 §3 defines
the field semantics and the `flags` bitfield; this catalog owns only
the layout.

## Open items

1. Per-`RawInputKind` payload struct layouts (§7).
2. C-header generator; offset/size golden test apparatus.
3. Whether `ByteSlice` allows `data == NULL` with `len == 0`
   (current default: yes — empty slice).
4. Whether enum variants with payloads use `#[repr(C, u8)]` plus
   union-of-payloads or separate per-variant types. Current default:
   the former for `FocusEntryWire`-style, the latter for
   higher-fanout cases.

## Conformance

| Behaviour | Fixture |
|---|---|
| Layout stability | Golden offset/size + alignment tests, per-target-triple, for every type in §§2..9. |
| `FrameHeader` wire bytes (§12) | Wire-bytes fixture: the 16-byte little-endian prefix round-trips through encode/decode; the 7.3 worked `Hello` frame is the permanent golden. |
| Generator parity | Generated C header round-trips through the catalog without divergence. |
| ABI bump discipline | CI gate that flags any `#[repr(C)]` field add/remove without an `AbiVersion.major` bump. |
