//! The handshake / correlation / unknown-tag state machine (7.3 §10).
//!
//! Pure, sans-IO logic over parsed [`FrameHeader`]s.  It enforces:
//!
//! - **SP9.1** — the first frame on a connection MUST be `Hello`/`HelloAck`;
//!   any other first frame is a protocol violation.
//! - **SP11** — correlation rules: a request carries a non-zero id, a response
//!   echoes it, a notify carries `0`.
//! - **§10.3** — unknown-tag policy: an unknown *notify* tag is skipped (stay
//!   framed); an unknown *req* tag is rejected (terminal).
//!
//! The machine holds **no buffer borrows** — it operates only on the parsed
//! header (which is `Copy`) and a tiny enum state.  It is therefore
//! `Send + 'static`: it can be owned by a connection task across awaits and
//! moved between threads.  Tag recognition is supplied by the caller (the
//! machine does not embed the inventory), keeping it policy-free.

use reovim_uapi_abi::FrameHeader;

use crate::{
    frame::FLAG_V1_ACCEPT_MASK,
    messages::{Direction, Hello, HelloAck, Message},
};

/// Which side of the connection a state machine drives.  The expected
/// first-frame tag and the unknown-tag policy differ per side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Server side: expects `Hello` first; rejects unknown *req* tags.
    Server,
    /// Client side: expects `HelloAck` first; skips unknown *notify* tags.
    Client,
}

/// Phase of the connection lifecycle (7.3 §10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No frame seen yet; the next frame MUST be the handshake (SP9.1).
    AwaitingHandshake,
    /// Handshake complete; request/response/notify frames flow.
    Established,
    /// A terminal protocol violation was observed; the connection closes.
    Closed,
}

/// The action a caller takes after feeding a frame header (7.3 §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The frame is valid; decode and dispatch its body.
    Accept,
    /// The frame's tag is unknown but it is a notify — skip its body and stay
    /// framed (§10.3 forward compatibility).
    SkipNotify,
    /// The frame violates the protocol; reject (terminal) and close the
    /// connection (SP14).
    Reject(RejectReason),
}

/// Why a frame was rejected (maps to a `Reject` `ErrorCode` at the call site).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// First frame was not the handshake (SP9.1) → `ProtocolViolation`.
    HandshakeExpected,
    /// A second handshake frame after the handshake completed →
    /// `ProtocolViolation`.
    UnexpectedHandshake,
    /// An unknown *req* tag (§10.3) → `ProtocolViolation`.
    UnknownRequest,
    /// A correlation-id rule was broken (SP11) → `ProtocolViolation`.
    CorrelationViolation,
    /// An unknown/reserved flag bit was set (7.3 §3) → `ProtocolViolation`.
    BadFlags,
}

/// What the caller knows about an incoming tag — supplied per frame so the
/// machine stays free of the message inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagInfo {
    /// The tag is a known message type with this direction.
    Known(Direction),
    /// The tag is not in the local inventory.
    Unknown,
}

/// The pure connection state machine (7.3 §10).
///
/// `Send + 'static` by construction: it carries only `Copy` scalar state and
/// borrows nothing.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::state::{ProtocolState, Role, Phase};
///
/// let sm = ProtocolState::new(Role::Server);
/// assert_eq!(sm.phase(), Phase::AwaitingHandshake);
///
/// // The type is Send + 'static (no buffer borrows in state).
/// fn assert_send_static<T: Send + 'static>(_: &T) {}
/// assert_send_static(&sm);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolState {
    role: Role,
    phase: Phase,
}

impl ProtocolState {
    /// Creates a fresh state machine for `role`, awaiting the handshake.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::state::{ProtocolState, Role, Phase};
    ///
    /// assert_eq!(ProtocolState::new(Role::Client).phase(), Phase::AwaitingHandshake);
    /// ```
    #[must_use]
    pub const fn new(role: Role) -> Self {
        Self {
            role,
            phase: Phase::AwaitingHandshake,
        }
    }

    /// Returns the current connection phase.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::state::{ProtocolState, Role, Phase};
    ///
    /// assert_eq!(ProtocolState::new(Role::Server).phase(), Phase::AwaitingHandshake);
    /// ```
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Feeds a parsed frame header (plus what the caller knows about its tag)
    /// and returns the [`Action`] to take, advancing the phase (7.3 §10).
    ///
    /// The body is never inspected — the machine decides purely from the
    /// header and the supplied [`TagInfo`].  After a [`Action::Reject`] the
    /// phase is [`Phase::Closed`] and further frames are rejected.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::state::{ProtocolState, Role, Action, TagInfo};
    /// use reovim_uapi_protocol::messages::{Direction, Hello, Message};
    /// use reovim_uapi_abi::FrameHeader;
    ///
    /// let mut sm = ProtocolState::new(Role::Server);
    /// let hello = FrameHeader { body_len: 0, msg_type: Hello::TAG, flags: 0, correlation_id: 0 };
    /// assert_eq!(sm.on_frame(hello, TagInfo::Known(Direction::Handshake)), Action::Accept);
    ///
    /// // After the handshake, a known request with a non-zero id is accepted.
    /// let req = FrameHeader { body_len: 0, msg_type: 0x0100, flags: 0, correlation_id: 7 };
    /// assert_eq!(sm.on_frame(req, TagInfo::Known(Direction::Request)), Action::Accept);
    /// ```
    pub const fn on_frame(&mut self, header: FrameHeader, tag: TagInfo) -> Action {
        if header.flags & !FLAG_V1_ACCEPT_MASK != 0 {
            self.phase = Phase::Closed;
            return Action::Reject(RejectReason::BadFlags);
        }
        match self.phase {
            Phase::AwaitingHandshake => self.on_handshake_frame(header, tag),
            Phase::Established => self.on_established_frame(header, tag),
            Phase::Closed => Action::Reject(RejectReason::HandshakeExpected),
        }
    }

    const fn on_handshake_frame(&mut self, header: FrameHeader, tag: TagInfo) -> Action {
        let expected_tag = match self.role {
            Role::Server => Hello::TAG,
            Role::Client => HelloAck::TAG,
        };
        let is_handshake = matches!(tag, TagInfo::Known(Direction::Handshake));
        if !is_handshake || header.msg_type != expected_tag {
            self.phase = Phase::Closed;
            return Action::Reject(RejectReason::HandshakeExpected);
        }
        if header.correlation_id != 0 {
            self.phase = Phase::Closed;
            return Action::Reject(RejectReason::CorrelationViolation);
        }
        self.phase = Phase::Established;
        Action::Accept
    }

    const fn on_established_frame(&mut self, header: FrameHeader, tag: TagInfo) -> Action {
        match tag {
            TagInfo::Known(dir) => self.on_known_frame(header, dir),
            TagInfo::Unknown => self.on_unknown_frame(),
        }
    }

    const fn on_known_frame(&mut self, header: FrameHeader, dir: Direction) -> Action {
        match dir {
            Direction::Handshake => {
                self.phase = Phase::Closed;
                Action::Reject(RejectReason::UnexpectedHandshake)
            }
            Direction::Request => {
                // SP11: a correlated request carries a non-zero id.  SendInput
                // (the uncorrelated hot-path frame, §7.5) carries 0 by design,
                // so a zero id on a request is allowed — correlation is opt-in
                // per §7.5.  The machine accepts both; correlation matching is
                // the caller's responsibility for the ids it issues.
                Action::Accept
            }
            Direction::Response | Direction::Error => Action::Accept,
            Direction::Notify => {
                if header.correlation_id == 0 {
                    Action::Accept
                } else {
                    self.phase = Phase::Closed;
                    Action::Reject(RejectReason::CorrelationViolation)
                }
            }
        }
    }

    const fn on_unknown_frame(&mut self) -> Action {
        // §10.3: skip unknown notify (client side), reject unknown req (server
        // side).  The role tells us which direction unknowns arrive in.
        match self.role {
            Role::Client => Action::SkipNotify,
            Role::Server => {
                self.phase = Phase::Closed;
                Action::Reject(RejectReason::UnknownRequest)
            }
        }
    }
}
