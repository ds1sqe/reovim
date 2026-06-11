//! The 36 hand-written message structs (7.3 §7 inventory).
//!
//! Each message is a borrowing view (`Msg<'a>`, SP17): fixed-width fields
//! decode by value, variable-length fields borrow the input buffer.  Every
//! message provides:
//!
//! - `const TAG: u16` — the §7 message-type tag (frozen forever, AB15).
//! - `const DIRECTION: Direction` — the §5 wire direction.
//! - `encoded_size(&self) -> usize` — exact wire size of the body.
//! - `encode(&self, out: &mut [u8]) -> Result<usize, ErrorCode>` — writes the
//!   body, returns byte count; [`ErrorCode::BufferTooSmall`] (nothing written
//!   past the failure) on a short buffer (SP13).
//! - `decode(buf: &'a [u8]) -> Result<Self, ErrorCode>` — validates the whole
//!   body then returns a borrowing view (SP17).
//!
//! Fields encode in the order of the §7 body table with no padding (SP13).
//!
//! ## Frame-cap enforcement (SP17)
//!
//! [`encode_capped`] refuses a value whose `encoded_size()` exceeds the SP10
//! `wire-max-frame-bytes` bound with [`ErrorCode::ResourceExhausted`], writing
//! nothing — an illegal over-cap frame body is never produced.  Plain `encode`
//! is the cap-agnostic primitive; callers that hold a cap use `encode_capped`.

use reovim_uapi_abi::{ErrorCode, input::RawInputKind};

use crate::{
    codec::{Decoder, Encoder, LEN_PREFIX, bytes_size},
    view::{CarrierList, DomainEntryList, RawInputList, StrList},
};

/// Wire direction of a message type (7.3 §5).  The direction is a property of
/// the tag, not a wire field.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::messages::{Direction, Hello, Message};
///
/// assert_eq!(Hello::DIRECTION, Direction::Handshake);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Handshake / lifecycle (`0x0001..=0x00FF`), `correlation_id == 0`.
    Handshake,
    /// Request, client → server (`0x0100..=0x01FF`).
    Request,
    /// Response, server → client (`0x0200..=0x02FF`), echoes the request id.
    Response,
    /// Notification, server → client (`0x0300..=0x03FF`), `correlation_id == 0`.
    Notify,
    /// Error / reject (`0xFF00..=0xFFFF`).
    Error,
}

/// Maps a `u8` to the [`RawInputKind`] discriminant; an unknown value fails
/// [`ErrorCode::InvalidArgument`] (§6.1 unknown discriminant in req direction).
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::messages::raw_input_kind_from_u8;
/// use reovim_uapi_abi::input::RawInputKind;
///
/// assert_eq!(raw_input_kind_from_u8(1).unwrap(), RawInputKind::Key);
/// assert!(raw_input_kind_from_u8(200).is_err());
/// ```
///
/// # Errors
///
/// Fails with [`ErrorCode::InvalidArgument`] under the conditions described above.
pub const fn raw_input_kind_from_u8(v: u8) -> Result<RawInputKind, ErrorCode> {
    match v {
        1 => Ok(RawInputKind::Key),
        2 => Ok(RawInputKind::Mouse),
        3 => Ok(RawInputKind::Text),
        4 => Ok(RawInputKind::Paste),
        5 => Ok(RawInputKind::Web),
        6 => Ok(RawInputKind::Trigger),
        7 => Ok(RawInputKind::Ime),
        _ => Err(ErrorCode::InvalidArgument),
    }
}

/// Maps an [`ErrorCode`] back from its `i32` discriminant; an out-of-range
/// value maps to [`ErrorCode::Generic`] (7.3 §9, AB14 — a `Reject` is already
/// the error path, so the frame is not rejected).
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::messages::error_code_from_i32;
/// use reovim_uapi_abi::ErrorCode;
///
/// assert_eq!(error_code_from_i32(0), ErrorCode::Ok);
/// assert_eq!(error_code_from_i32(26), ErrorCode::BufferTooSmall);
/// assert_eq!(error_code_from_i32(9999), ErrorCode::Generic);
/// assert_eq!(error_code_from_i32(-1), ErrorCode::Generic);
/// ```
pub const fn error_code_from_i32(v: i32) -> ErrorCode {
    match v {
        0 => ErrorCode::Ok,
        2 => ErrorCode::IncompatibleAbi,
        3 => ErrorCode::IncompatibleApi,
        4 => ErrorCode::NotFound,
        5 => ErrorCode::Conflict,
        6 => ErrorCode::InvalidArgument,
        7 => ErrorCode::ResourceExhausted,
        8 => ErrorCode::ProtocolViolation,
        9 => ErrorCode::Busy,
        10 => ErrorCode::Stale,
        11 => ErrorCode::PermissionDenied,
        12 => ErrorCode::Panic,
        13 => ErrorCode::Cancelled,
        14 => ErrorCode::Timeout,
        15 => ErrorCode::SchemaInvalid,
        16 => ErrorCode::NamespaceConflict,
        17 => ErrorCode::IllegalTrustClass,
        18 => ErrorCode::ConfigSliceTooLarge,
        19 => ErrorCode::Utf8Invalid,
        20 => ErrorCode::IllegalProjectHostSection,
        21 => ErrorCode::IllegalKind,
        22 => ErrorCode::ShortVtable,
        23 => ErrorCode::CodecGone,
        24 => ErrorCode::NotActive,
        25 => ErrorCode::RollbackFailed,
        26 => ErrorCode::BufferTooSmall,
        // 1 (Generic) and any out-of-range discriminant both degrade to Generic.
        _ => ErrorCode::Generic,
    }
}

/// Encodes `msg` into `out`, but refuses a body that would exceed `cap`
/// (`wire-max-frame-bytes`) with [`ErrorCode::ResourceExhausted`], writing
/// nothing (SP17).
///
/// This is the codec-boundary cap enforcement: an over-cap value is not a
/// legal frame body, so it is never serialized.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::messages::{encode_capped, Detach, Message};
/// use reovim_uapi_abi::ErrorCode;
///
/// let msg = Detach { reason: "bye" };
/// let mut buf = [0u8; 32];
/// // a tiny cap rejects the value before writing
/// assert_eq!(encode_capped(&msg, &mut buf, 2), Err(ErrorCode::ResourceExhausted));
/// // a sufficient cap encodes normally
/// let n = encode_capped(&msg, &mut buf, 1024).unwrap();
/// assert_eq!(n, msg.encoded_size());
/// ```
///
/// # Errors
///
/// Fails with [`ErrorCode::ResourceExhausted`] under the conditions described above.
pub fn encode_capped<M: Message>(msg: &M, out: &mut [u8], cap: usize) -> Result<usize, ErrorCode> {
    if msg.encoded_size() > cap {
        return Err(ErrorCode::ResourceExhausted);
    }
    msg.encode(out)
}

/// Common shape of every §7 message struct: tag, direction, and the codec
/// triple.  Implemented by all 36 message types so generic helpers
/// (e.g. [`encode_capped`]) work uniformly.
pub trait Message {
    /// The §7 message-type tag (frozen forever, AB15).
    const TAG: u16;
    /// The §5 wire direction.
    const DIRECTION: Direction;

    /// Exact wire size of the encoded body, in bytes.
    fn encoded_size(&self) -> usize;

    /// Encodes the body into `out`; returns the byte count.  A short buffer
    /// fails [`ErrorCode::BufferTooSmall`] (SP13) — checked HERE, up front,
    /// so nothing is written past a failure (the per-message
    /// [`encode_body`](Message::encode_body) only runs once the buffer is
    /// known to fit).
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] when `out` cannot hold
    /// [`encoded_size`](Message::encoded_size) bytes, and with
    /// [`ErrorCode::ResourceExhausted`] where a value exceeds a spec cap.
    fn encode(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        if out.len() < self.encoded_size() {
            return Err(ErrorCode::BufferTooSmall);
        }
        self.encode_body(out)
    }

    /// The per-message field encoding, in §7 body-table order.  Called by
    /// [`encode`](Message::encode) AFTER the SP13 size guard; calling it
    /// directly skips the nothing-written-on-failure guarantee.
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ResourceExhausted`] where a value exceeds a
    /// spec cap ([`ErrorCode::BufferTooSmall`] cannot occur through
    /// [`encode`](Message::encode), which pre-sizes the buffer).
    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode>;
}

/// Validates that a `list<str>` body region is well-formed and returns the
/// borrowing [`StrList`] view plus the bytes consumed (SP17: validate first).
fn decode_str_list<'a>(dec: &mut Decoder<'a>) -> Result<StrList<'a>, ErrorCode> {
    // Re-read the field as a slice we can hand to the view, validating each
    // element on the way so later traversal cannot fail.
    let mut probe = Decoder::new(dec.remaining_slice());
    let count = probe.get_u32()?;
    for _ in 0..count {
        probe.get_str()?;
    }
    let consumed = dec.remaining() - probe.remaining();
    let field = dec.take_exact(consumed)?;
    let body = &field[LEN_PREFIX..];
    Ok(StrList::from_parts(body, count))
}

/// Validates and returns a borrowing [`RawInputList`] view (SP17).
fn decode_rawinput_list<'a>(dec: &mut Decoder<'a>) -> Result<RawInputList<'a>, ErrorCode> {
    let mut probe = Decoder::new(dec.remaining_slice());
    let count = probe.get_u32()?;
    for _ in 0..count {
        let kind = probe.get_u8()?;
        raw_input_kind_from_u8(kind)?;
        probe.get_bytes()?;
    }
    let consumed = dec.remaining() - probe.remaining();
    let field = dec.take_exact(consumed)?;
    Ok(RawInputList::from_parts(&field[LEN_PREFIX..], count))
}

/// Validates and returns a borrowing [`DomainEntryList`] view (SP17).
fn decode_domain_list<'a>(dec: &mut Decoder<'a>) -> Result<DomainEntryList<'a>, ErrorCode> {
    let mut probe = Decoder::new(dec.remaining_slice());
    let count = probe.get_u32()?;
    for _ in 0..count {
        probe.get_str()?;
        probe.get_u32()?;
        probe.get_bool()?;
        probe.get_bool()?;
    }
    let consumed = dec.remaining() - probe.remaining();
    let field = dec.take_exact(consumed)?;
    Ok(DomainEntryList::from_parts(&field[LEN_PREFIX..], count))
}

/// Validates and returns a borrowing [`CarrierList`] view (SP17).
fn decode_carrier_list<'a>(dec: &mut Decoder<'a>) -> Result<CarrierList<'a>, ErrorCode> {
    let mut probe = Decoder::new(dec.remaining_slice());
    let count = probe.get_u32()?;
    for _ in 0..count {
        probe.get_u8_array8()?;
        probe.get_bytes()?;
    }
    let consumed = dec.remaining() - probe.remaining();
    let field = dec.take_exact(consumed)?;
    Ok(CarrierList::from_parts(&field[LEN_PREFIX..], count))
}

/// Encoded size of a `list<str>` from an iterator of its elements.
fn str_list_size<'a>(items: impl IntoIterator<Item = &'a str>) -> usize {
    LEN_PREFIX
        + items
            .into_iter()
            .map(|s| bytes_size(s.len()))
            .sum::<usize>()
}

/// Encodes a `list<str>` from an iterator of its elements.
fn put_str_list<'a>(
    enc: &mut Encoder<'_>,
    count: u32,
    items: impl IntoIterator<Item = &'a str>,
) -> Result<(), ErrorCode> {
    enc.put_u32(count)?;
    for s in items {
        enc.put_str(s)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Handshake (0x0001..=0x00FF)
// ---------------------------------------------------------------------------

/// `Hello` (tag `0x0001`, handshake) — the first client frame (7.3 §10.1).
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::messages::{Hello, Message};
/// use reovim_uapi_protocol::view::StrList;
///
/// let caps = [0x00, 0x00, 0x00, 0x00];
/// let codecs = [0x00, 0x00, 0x00, 0x00];
/// let msg = Hello {
///     protocol_major: 1,
///     protocol_minor: 0,
///     caps: StrList::from_validated(&caps),
///     domain_codecs: StrList::from_validated(&codecs),
/// };
/// let mut buf = [0u8; 64];
/// let n = msg.encode(&mut buf).unwrap();
/// let back = Hello::decode(&buf[..n]).unwrap();
/// assert_eq!(back.protocol_major, 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hello<'a> {
    /// Protocol major version.
    pub protocol_major: u16,
    /// Protocol minor version.
    pub protocol_minor: u16,
    /// Requested capability names.
    pub caps: StrList<'a>,
    /// Requested domain-codec names.
    pub domain_codecs: StrList<'a>,
}

impl<'a> Hello<'a> {
    /// Decodes a `Hello` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::Hello;
    ///
    /// // major=1, minor=0, caps=[], domain_codecs=[]
    /// let body = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    /// let h = Hello::decode(&body).unwrap();
    /// assert_eq!(h.protocol_major, 1);
    /// assert!(h.caps.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let protocol_major = dec.get_u16()?;
        let protocol_minor = dec.get_u16()?;
        let caps = decode_str_list(&mut dec)?;
        let domain_codecs = decode_str_list(&mut dec)?;
        Ok(Self {
            protocol_major,
            protocol_minor,
            caps,
            domain_codecs,
        })
    }
}

impl Message for Hello<'_> {
    const TAG: u16 = 0x0001;
    const DIRECTION: Direction = Direction::Handshake;

    fn encoded_size(&self) -> usize {
        2 + 2 + str_list_size(self.caps) + str_list_size(self.domain_codecs)
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u16(self.protocol_major)?;
        enc.put_u16(self.protocol_minor)?;
        put_str_list(&mut enc, u32_len(self.caps.len())?, self.caps)?;
        put_str_list(&mut enc, u32_len(self.domain_codecs.len())?, self.domain_codecs)?;
        Ok(enc.position())
    }
}

/// `HelloAck` (tag `0x0002`, handshake) — the server reply (7.3 §10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelloAck<'a> {
    /// Protocol major version.
    pub protocol_major: u16,
    /// Protocol minor version.
    pub protocol_minor: u16,
    /// Granted capability names.
    pub granted_caps: StrList<'a>,
    /// Server name.
    pub server_name: &'a str,
}

impl<'a> HelloAck<'a> {
    /// Decodes a `HelloAck` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::HelloAck;
    ///
    /// // major=1, minor=0, granted_caps=[], server_name="s"
    /// let body = [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, b's'];
    /// let a = HelloAck::decode(&body).unwrap();
    /// assert_eq!(a.server_name, "s");
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let protocol_major = dec.get_u16()?;
        let protocol_minor = dec.get_u16()?;
        let granted_caps = decode_str_list(&mut dec)?;
        let server_name = dec.get_str()?;
        Ok(Self {
            protocol_major,
            protocol_minor,
            granted_caps,
            server_name,
        })
    }
}

impl Message for HelloAck<'_> {
    const TAG: u16 = 0x0002;
    const DIRECTION: Direction = Direction::Handshake;

    fn encoded_size(&self) -> usize {
        2 + 2 + str_list_size(self.granted_caps) + bytes_size(self.server_name.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u16(self.protocol_major)?;
        enc.put_u16(self.protocol_minor)?;
        put_str_list(&mut enc, u32_len(self.granted_caps.len())?, self.granted_caps)?;
        enc.put_str(self.server_name)?;
        Ok(enc.position())
    }
}

// ---------------------------------------------------------------------------
// Requests (0x0100..=0x01FF)
// ---------------------------------------------------------------------------

/// `Attach` (tag `0x0100`, req) — attach to a session (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attach<'a> {
    /// Target session name.
    pub session_name: &'a str,
    /// Opaque auth bytes.
    pub auth: &'a [u8],
    /// Requested capability names.
    pub caps: StrList<'a>,
    /// Requested domain-codec names.
    pub domain_codecs: StrList<'a>,
}

impl<'a> Attach<'a> {
    /// Decodes an `Attach` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::Attach;
    ///
    /// // session="s", auth=[], caps=[], domain_codecs=[]
    /// let body = [1, 0, 0, 0, b's', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    /// let a = Attach::decode(&body).unwrap();
    /// assert_eq!(a.session_name, "s");
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let session_name = dec.get_str()?;
        let auth = dec.get_bytes()?;
        let caps = decode_str_list(&mut dec)?;
        let domain_codecs = decode_str_list(&mut dec)?;
        Ok(Self {
            session_name,
            auth,
            caps,
            domain_codecs,
        })
    }
}

impl Message for Attach<'_> {
    const TAG: u16 = 0x0100;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        bytes_size(self.session_name.len())
            + bytes_size(self.auth.len())
            + str_list_size(self.caps)
            + str_list_size(self.domain_codecs)
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_str(self.session_name)?;
        enc.put_bytes(self.auth)?;
        put_str_list(&mut enc, u32_len(self.caps.len())?, self.caps)?;
        put_str_list(&mut enc, u32_len(self.domain_codecs.len())?, self.domain_codecs)?;
        Ok(enc.position())
    }
}

/// `Detach` (tag `0x0101`, req) — detach with a reason (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detach<'a> {
    /// Human-readable detach reason.
    pub reason: &'a str,
}

impl<'a> Detach<'a> {
    /// Decodes a `Detach` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::Detach;
    ///
    /// let body = [2, 0, 0, 0, b'h', b'i'];
    /// assert_eq!(Detach::decode(&body).unwrap().reason, "hi");
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let reason = dec.get_str()?;
        Ok(Self { reason })
    }
}

impl Message for Detach<'_> {
    const TAG: u16 = 0x0101;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        bytes_size(self.reason.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_str(self.reason)?;
        Ok(enc.position())
    }
}

/// `SendInput` (tag `0x0102`, req) — uncorrelated hot-path input batch
/// (7.3 §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendInput<'a> {
    /// Originating client id.
    pub client_id: u64,
    /// Target buffer id.
    pub buffer_id: u64,
    /// Target window id.
    pub window_id: u64,
    /// The input batch.
    pub inputs: RawInputList<'a>,
}

impl<'a> SendInput<'a> {
    /// Decodes a `SendInput` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::SendInput;
    ///
    /// // client_id, buffer_id, window_id = 0; inputs count = 0
    /// let body = [0u8; 28];
    /// let s = SendInput::decode(&body).unwrap();
    /// assert!(s.inputs.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] / [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let client_id = dec.get_u64()?;
        let buffer_id = dec.get_u64()?;
        let window_id = dec.get_u64()?;
        let inputs = decode_rawinput_list(&mut dec)?;
        Ok(Self {
            client_id,
            buffer_id,
            window_id,
            inputs,
        })
    }

    fn inputs_size(&self) -> usize {
        LEN_PREFIX
            + self
                .inputs
                .iter()
                .map(|i| 1 + bytes_size(i.payload.len()))
                .sum::<usize>()
    }
}

impl Message for SendInput<'_> {
    const TAG: u16 = 0x0102;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        8 + 8 + 8 + self.inputs_size()
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u64(self.client_id)?;
        enc.put_u64(self.buffer_id)?;
        enc.put_u64(self.window_id)?;
        enc.put_u32(u32_len(self.inputs.len())?)?;
        for item in &self.inputs {
            enc.put_u8(item.kind as u8)?;
            enc.put_bytes(item.payload)?;
        }
        Ok(enc.position())
    }
}

/// `SwitchSession` (tag `0x0103`, req) — pivot to another session (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchSession<'a> {
    /// Target session name.
    pub session_name: &'a str,
}

impl<'a> SwitchSession<'a> {
    /// Decodes a `SwitchSession` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::SwitchSession;
    ///
    /// let body = [1, 0, 0, 0, b'x'];
    /// assert_eq!(SwitchSession::decode(&body).unwrap().session_name, "x");
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        Ok(Self {
            session_name: dec.get_str()?,
        })
    }
}

impl Message for SwitchSession<'_> {
    const TAG: u16 = 0x0103;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        bytes_size(self.session_name.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_str(self.session_name)?;
        Ok(enc.position())
    }
}

/// `DestroySession` (tag `0x0104`, req) — destroy a session (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestroySession<'a> {
    /// Target session name.
    pub session_name: &'a str,
    /// Force teardown of unresponsive clients.
    pub force: bool,
}

impl<'a> DestroySession<'a> {
    /// Decodes a `DestroySession` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::DestroySession;
    ///
    /// let body = [1, 0, 0, 0, b'x', 1];
    /// let d = DestroySession::decode(&body).unwrap();
    /// assert_eq!(d.session_name, "x");
    /// assert!(d.force);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let session_name = dec.get_str()?;
        let force = dec.get_bool()?;
        Ok(Self {
            session_name,
            force,
        })
    }
}

impl Message for DestroySession<'_> {
    const TAG: u16 = 0x0104;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        bytes_size(self.session_name.len()) + 1
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_str(self.session_name)?;
        enc.put_bool(self.force)?;
        Ok(enc.position())
    }
}

/// `RenameSession` (tag `0x0105`, req) — rename a session (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenameSession<'a> {
    /// Old session name.
    pub old_name: &'a str,
    /// New session name.
    pub new_name: &'a str,
}

impl<'a> RenameSession<'a> {
    /// Decodes a `RenameSession` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::RenameSession;
    ///
    /// let body = [1, 0, 0, 0, b'a', 1, 0, 0, 0, b'b'];
    /// let r = RenameSession::decode(&body).unwrap();
    /// assert_eq!((r.old_name, r.new_name), ("a", "b"));
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let old_name = dec.get_str()?;
        let new_name = dec.get_str()?;
        Ok(Self { old_name, new_name })
    }
}

impl Message for RenameSession<'_> {
    const TAG: u16 = 0x0105;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        bytes_size(self.old_name.len()) + bytes_size(self.new_name.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_str(self.old_name)?;
        enc.put_str(self.new_name)?;
        Ok(enc.position())
    }
}

/// `DebugRead` (tag `0x0106`, req) — read a debug channel (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugRead<'a> {
    /// Debug operation kind.
    pub op_kind: u32,
    /// Opaque op payload.
    pub payload: &'a [u8],
    /// Whether to subscribe to streamed events.
    pub subscribe: bool,
}

impl<'a> DebugRead<'a> {
    /// Decodes a `DebugRead` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::DebugRead;
    ///
    /// let body = [7, 0, 0, 0, 0, 0, 0, 0, 1];
    /// let d = DebugRead::decode(&body).unwrap();
    /// assert_eq!(d.op_kind, 7);
    /// assert!(d.subscribe);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let op_kind = dec.get_u32()?;
        let payload = dec.get_bytes()?;
        let subscribe = dec.get_bool()?;
        Ok(Self {
            op_kind,
            payload,
            subscribe,
        })
    }
}

impl Message for DebugRead<'_> {
    const TAG: u16 = 0x0106;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        4 + bytes_size(self.payload.len()) + 1
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u32(self.op_kind)?;
        enc.put_bytes(self.payload)?;
        enc.put_bool(self.subscribe)?;
        Ok(enc.position())
    }
}

/// `DebugDrive` (tag `0x0107`, req) — drive a debug operation (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugDrive<'a> {
    /// Debug operation kind.
    pub op_kind: u32,
    /// Opaque op payload.
    pub payload: &'a [u8],
}

impl<'a> DebugDrive<'a> {
    /// Decodes a `DebugDrive` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::DebugDrive;
    ///
    /// let body = [9, 0, 0, 0, 0, 0, 0, 0];
    /// assert_eq!(DebugDrive::decode(&body).unwrap().op_kind, 9);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let op_kind = dec.get_u32()?;
        let payload = dec.get_bytes()?;
        Ok(Self { op_kind, payload })
    }
}

impl Message for DebugDrive<'_> {
    const TAG: u16 = 0x0107;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        4 + bytes_size(self.payload.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u32(self.op_kind)?;
        enc.put_bytes(self.payload)?;
        Ok(enc.position())
    }
}

/// `PkgSync` (tag `0x0108`, req) — sync package targets (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PkgSync<'a> {
    /// Target package names.
    pub targets: StrList<'a>,
    /// Plan-only, no mutation.
    pub dry_run: bool,
}

impl<'a> PkgSync<'a> {
    /// Decodes a `PkgSync` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::PkgSync;
    ///
    /// let body = [0, 0, 0, 0, 1];
    /// let p = PkgSync::decode(&body).unwrap();
    /// assert!(p.dry_run);
    /// assert!(p.targets.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let targets = decode_str_list(&mut dec)?;
        let dry_run = dec.get_bool()?;
        Ok(Self { targets, dry_run })
    }
}

impl Message for PkgSync<'_> {
    const TAG: u16 = 0x0108;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        str_list_size(self.targets) + 1
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        put_str_list(&mut enc, u32_len(self.targets.len())?, self.targets)?;
        enc.put_bool(self.dry_run)?;
        Ok(enc.position())
    }
}

/// `PkgVerify` (tag `0x0109`, req) — verify package targets (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PkgVerify<'a> {
    /// Target package names.
    pub targets: StrList<'a>,
}

impl<'a> PkgVerify<'a> {
    /// Decodes a `PkgVerify` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::PkgVerify;
    ///
    /// let body = [0, 0, 0, 0];
    /// assert!(PkgVerify::decode(&body).unwrap().targets.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        Ok(Self {
            targets: decode_str_list(&mut dec)?,
        })
    }
}

impl Message for PkgVerify<'_> {
    const TAG: u16 = 0x0109;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        str_list_size(self.targets)
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        put_str_list(&mut enc, u32_len(self.targets.len())?, self.targets)?;
        Ok(enc.position())
    }
}

/// `ConfigDump` (tag `0x010A`, req) — dump configuration (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigDump<'a> {
    /// Config namespace.
    pub namespace: &'a str,
    /// Whether to reveal secret values.
    pub show_secrets: bool,
}

impl<'a> ConfigDump<'a> {
    /// Decodes a `ConfigDump` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::ConfigDump;
    ///
    /// let body = [1, 0, 0, 0, b'k', 0];
    /// let c = ConfigDump::decode(&body).unwrap();
    /// assert_eq!(c.namespace, "k");
    /// assert!(!c.show_secrets);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let namespace = dec.get_str()?;
        let show_secrets = dec.get_bool()?;
        Ok(Self {
            namespace,
            show_secrets,
        })
    }
}

impl Message for ConfigDump<'_> {
    const TAG: u16 = 0x010A;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        bytes_size(self.namespace.len()) + 1
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_str(self.namespace)?;
        enc.put_bool(self.show_secrets)?;
        Ok(enc.position())
    }
}

/// `ConfigValidate` (tag `0x010B`, req) — validate a config blob (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigValidate<'a> {
    /// TOML config bytes.
    pub toml: &'a [u8],
    /// Config namespace.
    pub namespace: &'a str,
}

impl<'a> ConfigValidate<'a> {
    /// Decodes a `ConfigValidate` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::ConfigValidate;
    ///
    /// let body = [0, 0, 0, 0, 1, 0, 0, 0, b'n'];
    /// let c = ConfigValidate::decode(&body).unwrap();
    /// assert_eq!(c.namespace, "n");
    /// assert!(c.toml.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let toml = dec.get_bytes()?;
        let namespace = dec.get_str()?;
        Ok(Self { toml, namespace })
    }
}

impl Message for ConfigValidate<'_> {
    const TAG: u16 = 0x010B;
    const DIRECTION: Direction = Direction::Request;

    fn encoded_size(&self) -> usize {
        bytes_size(self.toml.len()) + bytes_size(self.namespace.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_bytes(self.toml)?;
        enc.put_str(self.namespace)?;
        Ok(enc.position())
    }
}

// ---------------------------------------------------------------------------
// Responses (0x0200..=0x02FF)
// ---------------------------------------------------------------------------

/// `AttachAck` (tag `0x0200`, resp) — attach result + domain table (7.3 §7, §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachAck<'a> {
    /// Assigned client id.
    pub client_id: u64,
    /// Initial domain table.
    pub domain_table: DomainEntryList<'a>,
}

impl<'a> AttachAck<'a> {
    /// Decodes an `AttachAck` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachAck;
    ///
    /// let body = [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    /// let a = AttachAck::decode(&body).unwrap();
    /// assert_eq!(a.client_id, 7);
    /// assert!(a.domain_table.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let client_id = dec.get_u64()?;
        let domain_table = decode_domain_list(&mut dec)?;
        Ok(Self {
            client_id,
            domain_table,
        })
    }

    fn domain_table_size(&self) -> usize {
        LEN_PREFIX
            + self
                .domain_table
                .iter()
                .map(|e| bytes_size(e.domain_name.len()) + 4 + 1 + 1)
                .sum::<usize>()
    }
}

impl Message for AttachAck<'_> {
    const TAG: u16 = 0x0200;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        8 + self.domain_table_size()
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u64(self.client_id)?;
        put_domain_list(&mut enc, u32_len(self.domain_table.len())?, &self.domain_table)?;
        Ok(enc.position())
    }
}

/// `DetachAck` (tag `0x0201`, resp) — empty acknowledgement (7.3 §7).
#[derive(Clone, Copy, Default)]
pub struct DetachAck;

impl DetachAck {
    /// Decodes the empty `DetachAck` body (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::DetachAck;
    ///
    /// assert!(DetachAck::decode(&[]).is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// Never fails today; the `Result` shape is the uniform SP13 decode contract.
    pub const fn decode(_buf: &[u8]) -> Result<Self, ErrorCode> {
        Ok(Self)
    }
}

impl Message for DetachAck {
    const TAG: u16 = 0x0201;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        0
    }

    fn encode_body(&self, _out: &mut [u8]) -> Result<usize, ErrorCode> {
        Ok(0)
    }
}

/// `InputAck` (tag `0x0202`, resp) — flow-control checkpoint (7.3 §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputAck {
    /// Cumulative accepted-input count since the last ack.
    pub accepted: u64,
    /// Cumulative dropped-input count since the last ack.
    pub dropped: u64,
}

impl InputAck {
    /// Decodes an `InputAck` body (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::InputAck;
    ///
    /// let body = [3, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
    /// let a = InputAck::decode(&body).unwrap();
    /// assert_eq!((a.accepted, a.dropped), (3, 1));
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &[u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let accepted = dec.get_u64()?;
        let dropped = dec.get_u64()?;
        Ok(Self { accepted, dropped })
    }
}

impl Message for InputAck {
    const TAG: u16 = 0x0202;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        16
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u64(self.accepted)?;
        enc.put_u64(self.dropped)?;
        Ok(enc.position())
    }
}

/// `SwitchSessionAck` (tag `0x0203`, resp) — empty; projection follows on
/// notify (7.3 §7).
#[derive(Clone, Copy, Default)]
pub struct SwitchSessionAck;

impl SwitchSessionAck {
    /// Decodes the empty `SwitchSessionAck` body (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::SwitchSessionAck;
    ///
    /// assert!(SwitchSessionAck::decode(&[]).is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// Never fails today; the `Result` shape is the uniform SP13 decode contract.
    pub const fn decode(_buf: &[u8]) -> Result<Self, ErrorCode> {
        Ok(Self)
    }
}

impl Message for SwitchSessionAck {
    const TAG: u16 = 0x0203;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        0
    }

    fn encode_body(&self, _out: &mut [u8]) -> Result<usize, ErrorCode> {
        Ok(0)
    }
}

/// `DestroySessionAck` (tag `0x0204`, resp) — empty acknowledgement (7.3 §7).
#[derive(Clone, Copy, Default)]
pub struct DestroySessionAck;

impl DestroySessionAck {
    /// Decodes the empty `DestroySessionAck` body (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::DestroySessionAck;
    ///
    /// assert!(DestroySessionAck::decode(&[]).is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// Never fails today; the `Result` shape is the uniform SP13 decode contract.
    pub const fn decode(_buf: &[u8]) -> Result<Self, ErrorCode> {
        Ok(Self)
    }
}

impl Message for DestroySessionAck {
    const TAG: u16 = 0x0204;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        0
    }

    fn encode_body(&self, _out: &mut [u8]) -> Result<usize, ErrorCode> {
        Ok(0)
    }
}

/// `RenameSessionAck` (tag `0x0205`, resp) — empty acknowledgement (7.3 §7).
#[derive(Clone, Copy, Default)]
pub struct RenameSessionAck;

impl RenameSessionAck {
    /// Decodes the empty `RenameSessionAck` body (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::RenameSessionAck;
    ///
    /// assert!(RenameSessionAck::decode(&[]).is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// Never fails today; the `Result` shape is the uniform SP13 decode contract.
    pub const fn decode(_buf: &[u8]) -> Result<Self, ErrorCode> {
        Ok(Self)
    }
}

impl Message for RenameSessionAck {
    const TAG: u16 = 0x0205;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        0
    }

    fn encode_body(&self, _out: &mut [u8]) -> Result<usize, ErrorCode> {
        Ok(0)
    }
}

/// `DebugReadEvent` (tag `0x0206`, resp) — a debug-read result chunk (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugReadEvent<'a> {
    /// Debug operation kind.
    pub op_kind: u32,
    /// Opaque result payload.
    pub payload: &'a [u8],
    /// Whether this is the final chunk.
    pub is_final: bool,
}

impl<'a> DebugReadEvent<'a> {
    /// Decodes a `DebugReadEvent` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::DebugReadEvent;
    ///
    /// let body = [7, 0, 0, 0, 0, 0, 0, 0, 1];
    /// let d = DebugReadEvent::decode(&body).unwrap();
    /// assert!(d.is_final);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let op_kind = dec.get_u32()?;
        let payload = dec.get_bytes()?;
        let is_final = dec.get_bool()?;
        Ok(Self {
            op_kind,
            payload,
            is_final,
        })
    }
}

impl Message for DebugReadEvent<'_> {
    const TAG: u16 = 0x0206;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        4 + bytes_size(self.payload.len()) + 1
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u32(self.op_kind)?;
        enc.put_bytes(self.payload)?;
        enc.put_bool(self.is_final)?;
        Ok(enc.position())
    }
}

/// `DebugDriveAck` (tag `0x0207`, resp) — a debug-drive result (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugDriveAck<'a> {
    /// Debug operation kind.
    pub op_kind: u32,
    /// Operation result code.
    pub result: ErrorCode,
    /// Opaque result payload.
    pub payload: &'a [u8],
}

impl<'a> DebugDriveAck<'a> {
    /// Decodes a `DebugDriveAck` body into a borrowing view; an out-of-range
    /// `result` code degrades to [`ErrorCode::Generic`] (7.3 §9).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::DebugDriveAck;
    /// use reovim_uapi_abi::ErrorCode;
    ///
    /// let body = [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    /// let d = DebugDriveAck::decode(&body).unwrap();
    /// assert_eq!(d.result, ErrorCode::Ok);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let op_kind = dec.get_u32()?;
        let result = error_code_from_i32(dec.get_i32()?);
        let payload = dec.get_bytes()?;
        Ok(Self {
            op_kind,
            result,
            payload,
        })
    }
}

impl Message for DebugDriveAck<'_> {
    const TAG: u16 = 0x0207;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        4 + 4 + bytes_size(self.payload.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u32(self.op_kind)?;
        enc.put_i32(self.result as i32)?;
        enc.put_bytes(self.payload)?;
        Ok(enc.position())
    }
}

/// `PkgSyncProgress` (tag `0x0208`, resp) — sync progress update (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PkgSyncProgress<'a> {
    /// Target package name.
    pub target: &'a str,
    /// Units done.
    pub done: u32,
    /// Total units.
    pub total: u32,
}

impl<'a> PkgSyncProgress<'a> {
    /// Decodes a `PkgSyncProgress` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::PkgSyncProgress;
    ///
    /// let body = [1, 0, 0, 0, b'p', 2, 0, 0, 0, 5, 0, 0, 0];
    /// let p = PkgSyncProgress::decode(&body).unwrap();
    /// assert_eq!((p.target, p.done, p.total), ("p", 2, 5));
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let target = dec.get_str()?;
        let done = dec.get_u32()?;
        let total = dec.get_u32()?;
        Ok(Self {
            target,
            done,
            total,
        })
    }
}

impl Message for PkgSyncProgress<'_> {
    const TAG: u16 = 0x0208;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        bytes_size(self.target.len()) + 4 + 4
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_str(self.target)?;
        enc.put_u32(self.done)?;
        enc.put_u32(self.total)?;
        Ok(enc.position())
    }
}

/// `PkgSyncDone` (tag `0x0209`, resp) — sync completion (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PkgSyncDone<'a> {
    /// Final result code.
    pub result: ErrorCode,
    /// Human-readable summary.
    pub summary: &'a str,
}

impl<'a> PkgSyncDone<'a> {
    /// Decodes a `PkgSyncDone` body into a borrowing view; an out-of-range
    /// `result` degrades to [`ErrorCode::Generic`] (7.3 §9).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::PkgSyncDone;
    /// use reovim_uapi_abi::ErrorCode;
    ///
    /// let body = [0, 0, 0, 0, 1, 0, 0, 0, b'k'];
    /// let p = PkgSyncDone::decode(&body).unwrap();
    /// assert_eq!(p.result, ErrorCode::Ok);
    /// assert_eq!(p.summary, "k");
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let result = error_code_from_i32(dec.get_i32()?);
        let summary = dec.get_str()?;
        Ok(Self { result, summary })
    }
}

impl Message for PkgSyncDone<'_> {
    const TAG: u16 = 0x0209;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        4 + bytes_size(self.summary.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_i32(self.result as i32)?;
        enc.put_str(self.summary)?;
        Ok(enc.position())
    }
}

/// `PkgVerifyAck` (tag `0x020A`, resp) — verify result + report (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PkgVerifyAck<'a> {
    /// Verify result code.
    pub result: ErrorCode,
    /// Opaque report bytes.
    pub report: &'a [u8],
}

impl<'a> PkgVerifyAck<'a> {
    /// Decodes a `PkgVerifyAck` body into a borrowing view; an out-of-range
    /// `result` degrades to [`ErrorCode::Generic`] (7.3 §9).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::PkgVerifyAck;
    /// use reovim_uapi_abi::ErrorCode;
    ///
    /// let body = [0, 0, 0, 0, 0, 0, 0, 0];
    /// let p = PkgVerifyAck::decode(&body).unwrap();
    /// assert_eq!(p.result, ErrorCode::Ok);
    /// assert!(p.report.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let result = error_code_from_i32(dec.get_i32()?);
        let report = dec.get_bytes()?;
        Ok(Self { result, report })
    }
}

impl Message for PkgVerifyAck<'_> {
    const TAG: u16 = 0x020A;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        4 + bytes_size(self.report.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_i32(self.result as i32)?;
        enc.put_bytes(self.report)?;
        Ok(enc.position())
    }
}

/// `ConfigDumpResponse` (tag `0x020B`, resp) — dumped TOML (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigDumpResponse<'a> {
    /// Dumped TOML config bytes.
    pub toml: &'a [u8],
}

impl<'a> ConfigDumpResponse<'a> {
    /// Decodes a `ConfigDumpResponse` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::ConfigDumpResponse;
    ///
    /// let body = [1, 0, 0, 0, 0xAB];
    /// assert_eq!(ConfigDumpResponse::decode(&body).unwrap().toml, &[0xAB]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        Ok(Self {
            toml: dec.get_bytes()?,
        })
    }
}

impl Message for ConfigDumpResponse<'_> {
    const TAG: u16 = 0x020B;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        bytes_size(self.toml.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_bytes(self.toml)?;
        Ok(enc.position())
    }
}

/// `ConfigValidateResponse` (tag `0x020C`, resp) — validate result (7.3 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigValidateResponse<'a> {
    /// Validation result code.
    pub result: ErrorCode,
    /// Human-readable detail.
    pub detail: &'a str,
}

impl<'a> ConfigValidateResponse<'a> {
    /// Decodes a `ConfigValidateResponse` body into a borrowing view; an
    /// out-of-range `result` degrades to [`ErrorCode::Generic`] (7.3 §9).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::ConfigValidateResponse;
    /// use reovim_uapi_abi::ErrorCode;
    ///
    /// let body = [0, 0, 0, 0, 0, 0, 0, 0];
    /// let c = ConfigValidateResponse::decode(&body).unwrap();
    /// assert_eq!(c.result, ErrorCode::Ok);
    /// assert_eq!(c.detail, "");
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let result = error_code_from_i32(dec.get_i32()?);
        let detail = dec.get_str()?;
        Ok(Self { result, detail })
    }
}

impl Message for ConfigValidateResponse<'_> {
    const TAG: u16 = 0x020C;
    const DIRECTION: Direction = Direction::Response;

    fn encoded_size(&self) -> usize {
        4 + bytes_size(self.detail.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_i32(self.result as i32)?;
        enc.put_str(self.detail)?;
        Ok(enc.position())
    }
}

// ---------------------------------------------------------------------------
// Notifications: the AttachEvent family (0x0301..=0x0309)
// ---------------------------------------------------------------------------

/// `AttachEvent::Frame` (tag `0x0301`, notify) — a full render frame (7.3 §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEventFrame<'a> {
    /// Source buffer id.
    pub buffer_id: u64,
    /// Source window id.
    pub window_id: u64,
    /// Opaque render-frame bytes.
    pub frame: &'a [u8],
}

impl<'a> AttachEventFrame<'a> {
    /// Decodes an `AttachEvent::Frame` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachEventFrame;
    ///
    /// let body = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    /// let e = AttachEventFrame::decode(&body).unwrap();
    /// assert!(e.frame.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let buffer_id = dec.get_u64()?;
        let window_id = dec.get_u64()?;
        let frame = dec.get_bytes()?;
        Ok(Self {
            buffer_id,
            window_id,
            frame,
        })
    }
}

impl Message for AttachEventFrame<'_> {
    const TAG: u16 = 0x0301;
    const DIRECTION: Direction = Direction::Notify;

    fn encoded_size(&self) -> usize {
        8 + 8 + bytes_size(self.frame.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u64(self.buffer_id)?;
        enc.put_u64(self.window_id)?;
        enc.put_bytes(self.frame)?;
        Ok(enc.position())
    }
}

/// `AttachEvent::Diff` (tag `0x0302`, notify) — a render diff (7.3 §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEventDiff<'a> {
    /// Source buffer id.
    pub buffer_id: u64,
    /// Source window id.
    pub window_id: u64,
    /// Opaque render-diff bytes.
    pub diff: &'a [u8],
}

impl<'a> AttachEventDiff<'a> {
    /// Decodes an `AttachEvent::Diff` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachEventDiff;
    ///
    /// let body = [0u8; 20];
    /// let e = AttachEventDiff::decode(&body).unwrap();
    /// assert!(e.diff.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let buffer_id = dec.get_u64()?;
        let window_id = dec.get_u64()?;
        let diff = dec.get_bytes()?;
        Ok(Self {
            buffer_id,
            window_id,
            diff,
        })
    }
}

impl Message for AttachEventDiff<'_> {
    const TAG: u16 = 0x0302;
    const DIRECTION: Direction = Direction::Notify;

    fn encoded_size(&self) -> usize {
        8 + 8 + bytes_size(self.diff.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u64(self.buffer_id)?;
        enc.put_u64(self.window_id)?;
        enc.put_bytes(self.diff)?;
        Ok(enc.position())
    }
}

/// `AttachEvent::Cursor` (tag `0x0303`, notify) — cursor carriers (7.3 §7.4,
/// §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEventCursor<'a> {
    /// Source buffer id.
    pub buffer_id: u64,
    /// Source window id.
    pub window_id: u64,
    /// Cursor carriers (an empty list is the empty cursor set, CR3).
    pub cursors: CarrierList<'a>,
}

impl<'a> AttachEventCursor<'a> {
    /// Decodes an `AttachEvent::Cursor` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachEventCursor;
    ///
    /// let body = [0u8; 20]; // ids = 0, cursors count = 0
    /// let e = AttachEventCursor::decode(&body).unwrap();
    /// assert!(e.cursors.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let buffer_id = dec.get_u64()?;
        let window_id = dec.get_u64()?;
        let cursors = decode_carrier_list(&mut dec)?;
        Ok(Self {
            buffer_id,
            window_id,
            cursors,
        })
    }

    fn cursors_size(&self) -> usize {
        LEN_PREFIX
            + self
                .cursors
                .iter()
                .map(|c| 8 + bytes_size(c.content.len()))
                .sum::<usize>()
    }
}

impl Message for AttachEventCursor<'_> {
    const TAG: u16 = 0x0303;
    const DIRECTION: Direction = Direction::Notify;

    fn encoded_size(&self) -> usize {
        8 + 8 + self.cursors_size()
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u64(self.buffer_id)?;
        enc.put_u64(self.window_id)?;
        enc.put_u32(u32_len(self.cursors.len())?)?;
        for c in &self.cursors {
            enc.put_u8_array8(&c.header)?;
            enc.put_bytes(c.content)?;
        }
        Ok(enc.position())
    }
}

/// `AttachEvent::Projection` (tag `0x0304`, notify) — a domain projection
/// (7.3 §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEventProjection<'a> {
    /// Source buffer id.
    pub buffer_id: u64,
    /// Opaque projection bytes.
    pub projection: &'a [u8],
}

impl<'a> AttachEventProjection<'a> {
    /// Decodes an `AttachEvent::Projection` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachEventProjection;
    ///
    /// let body = [0u8; 12];
    /// let e = AttachEventProjection::decode(&body).unwrap();
    /// assert!(e.projection.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let buffer_id = dec.get_u64()?;
        let projection = dec.get_bytes()?;
        Ok(Self {
            buffer_id,
            projection,
        })
    }
}

impl Message for AttachEventProjection<'_> {
    const TAG: u16 = 0x0304;
    const DIRECTION: Direction = Direction::Notify;

    fn encoded_size(&self) -> usize {
        8 + bytes_size(self.projection.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u64(self.buffer_id)?;
        enc.put_bytes(self.projection)?;
        Ok(enc.position())
    }
}

/// `AttachEvent::DomainTableDelta` (tag `0x0305`, notify) — codec
/// register/unregister deltas (7.3 §7.4, §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEventDomainTableDelta<'a> {
    /// Newly-added domain entries.
    pub added: DomainEntryList<'a>,
    /// Removed domain entries.
    pub removed: DomainEntryList<'a>,
}

impl<'a> AttachEventDomainTableDelta<'a> {
    /// Decodes an `AttachEvent::DomainTableDelta` body into a borrowing view
    /// (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachEventDomainTableDelta;
    ///
    /// let body = [0, 0, 0, 0, 0, 0, 0, 0]; // added = [], removed = []
    /// let e = AttachEventDomainTableDelta::decode(&body).unwrap();
    /// assert!(e.added.is_empty() && e.removed.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let added = decode_domain_list(&mut dec)?;
        let removed = decode_domain_list(&mut dec)?;
        Ok(Self { added, removed })
    }

    fn list_size(list: &DomainEntryList<'_>) -> usize {
        LEN_PREFIX
            + list
                .iter()
                .map(|e| bytes_size(e.domain_name.len()) + 4 + 1 + 1)
                .sum::<usize>()
    }
}

impl Message for AttachEventDomainTableDelta<'_> {
    const TAG: u16 = 0x0305;
    const DIRECTION: Direction = Direction::Notify;

    fn encoded_size(&self) -> usize {
        Self::list_size(&self.added) + Self::list_size(&self.removed)
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        put_domain_list(&mut enc, u32_len(self.added.len())?, &self.added)?;
        put_domain_list(&mut enc, u32_len(self.removed.len())?, &self.removed)?;
        Ok(enc.position())
    }
}

/// `AttachEvent::SessionPivot` (tag `0x0306`, notify) — session pivot (7.3 §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEventSessionPivot<'a> {
    /// New active session name.
    pub session_name: &'a str,
}

impl<'a> AttachEventSessionPivot<'a> {
    /// Decodes an `AttachEvent::SessionPivot` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachEventSessionPivot;
    ///
    /// let body = [1, 0, 0, 0, b's'];
    /// assert_eq!(AttachEventSessionPivot::decode(&body).unwrap().session_name, "s");
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        Ok(Self {
            session_name: dec.get_str()?,
        })
    }
}

impl Message for AttachEventSessionPivot<'_> {
    const TAG: u16 = 0x0306;
    const DIRECTION: Direction = Direction::Notify;

    fn encoded_size(&self) -> usize {
        bytes_size(self.session_name.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_str(self.session_name)?;
        Ok(enc.position())
    }
}

/// `AttachEvent::SessionDestroyed` (tag `0x0307`, notify) — session destroyed
/// (7.3 §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEventSessionDestroyed<'a> {
    /// Destroyed session name.
    pub session_name: &'a str,
}

impl<'a> AttachEventSessionDestroyed<'a> {
    /// Decodes an `AttachEvent::SessionDestroyed` body into a borrowing view
    /// (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachEventSessionDestroyed;
    ///
    /// let body = [1, 0, 0, 0, b's'];
    /// assert_eq!(AttachEventSessionDestroyed::decode(&body).unwrap().session_name, "s");
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        Ok(Self {
            session_name: dec.get_str()?,
        })
    }
}

impl Message for AttachEventSessionDestroyed<'_> {
    const TAG: u16 = 0x0307;
    const DIRECTION: Direction = Direction::Notify;

    fn encoded_size(&self) -> usize {
        bytes_size(self.session_name.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_str(self.session_name)?;
        Ok(enc.position())
    }
}

/// `AttachEvent::ClientLeft` (tag `0x0308`, notify) — a peer detached (7.3 §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEventClientLeft<'a> {
    /// The departed client id.
    pub client_id: u64,
    /// Human-readable reason.
    pub reason: &'a str,
}

impl<'a> AttachEventClientLeft<'a> {
    /// Decodes an `AttachEvent::ClientLeft` body into a borrowing view (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachEventClientLeft;
    ///
    /// let body = [3, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, b'x'];
    /// let e = AttachEventClientLeft::decode(&body).unwrap();
    /// assert_eq!((e.client_id, e.reason), (3, "x"));
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let client_id = dec.get_u64()?;
        let reason = dec.get_str()?;
        Ok(Self { client_id, reason })
    }
}

impl Message for AttachEventClientLeft<'_> {
    const TAG: u16 = 0x0308;
    const DIRECTION: Direction = Direction::Notify;

    fn encoded_size(&self) -> usize {
        8 + bytes_size(self.reason.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u64(self.client_id)?;
        enc.put_str(self.reason)?;
        Ok(enc.position())
    }
}

/// `AttachEvent::ServerDraining` (tag `0x0309`, notify) — shutdown drain (7.3
/// §7.4, SP8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEventServerDraining {
    /// Grace window before forced close, in milliseconds.
    pub grace_ms: u32,
}

impl AttachEventServerDraining {
    /// Decodes an `AttachEvent::ServerDraining` body (SP17).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::AttachEventServerDraining;
    ///
    /// let body = [0xE8, 0x03, 0, 0]; // 1000
    /// assert_eq!(AttachEventServerDraining::decode(&body).unwrap().grace_ms, 1000);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &[u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        Ok(Self {
            grace_ms: dec.get_u32()?,
        })
    }
}

impl Message for AttachEventServerDraining {
    const TAG: u16 = 0x0309;
    const DIRECTION: Direction = Direction::Notify;

    fn encoded_size(&self) -> usize {
        4
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_u32(self.grace_ms)?;
        Ok(enc.position())
    }
}

// ---------------------------------------------------------------------------
// Error (0xFF00..=0xFFFF)
// ---------------------------------------------------------------------------

/// `Reject` (tag `0xFF00`, error) — the single error frame (7.3 §9, SP14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reject<'a> {
    /// The error code.
    pub code: ErrorCode,
    /// Human-readable detail.
    pub detail: &'a str,
}

impl<'a> Reject<'a> {
    /// Decodes a `Reject` body into a borrowing view; an out-of-range `code`
    /// discriminant degrades to [`ErrorCode::Generic`] rather than rejecting
    /// the frame (7.3 §9).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::messages::Reject;
    /// use reovim_uapi_abi::ErrorCode;
    ///
    /// // code = 9999 (out of range) → Generic; detail = "x"
    /// let body = [0x0F, 0x27, 0, 0, 1, 0, 0, 0, b'x'];
    /// let r = Reject::decode(&body).unwrap();
    /// assert_eq!(r.code, ErrorCode::Generic);
    /// assert_eq!(r.detail, "x");
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn decode(buf: &'a [u8]) -> Result<Self, ErrorCode> {
        let mut dec = Decoder::new(buf);
        let code = error_code_from_i32(dec.get_i32()?);
        let detail = dec.get_str()?;
        Ok(Self { code, detail })
    }
}

impl Message for Reject<'_> {
    const TAG: u16 = 0xFF00;
    const DIRECTION: Direction = Direction::Error;

    fn encoded_size(&self) -> usize {
        4 + bytes_size(self.detail.len())
    }

    fn encode_body(&self, out: &mut [u8]) -> Result<usize, ErrorCode> {
        let mut enc = Encoder::new(out);
        enc.put_i32(self.code as i32)?;
        enc.put_str(self.detail)?;
        Ok(enc.position())
    }
}

// ---------------------------------------------------------------------------
// Shared encoders / helpers
// ---------------------------------------------------------------------------

/// Converts a `usize` element count to the `u32` wire count, failing
/// [`ErrorCode::ResourceExhausted`] when it overflows `u32` (a list that large
/// cannot be a legal frame body).
fn u32_len(n: usize) -> Result<u32, ErrorCode> {
    u32::try_from(n).map_err(|_| ErrorCode::ResourceExhausted)
}

/// Encodes a `list<DomainEntry>` from a borrowing view.
fn put_domain_list(
    enc: &mut Encoder<'_>,
    count: u32,
    list: &DomainEntryList<'_>,
) -> Result<(), ErrorCode> {
    enc.put_u32(count)?;
    for e in list {
        enc.put_str(e.domain_name)?;
        enc.put_u32(e.inner_id)?;
        enc.put_bool(e.has_display)?;
        enc.put_bool(e.has_semantic)?;
    }
    Ok(())
}
