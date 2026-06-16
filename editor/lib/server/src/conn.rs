//! Per-connection state (SP9: one client = one stream connection).
//!
//! [`ConnState`] tracks the connection lifecycle: whether the SP9.1 handshake
//! has been completed and whether `Attach` has been received (at most once
//! per connection — SP1). The `ProtocolState` machine from `uapi/protocol`
//! drives the wire-level handshake and correlation rules; `ConnState` tracks
//! the higher-level attach status.

use reovim_uapi_protocol::state::{ProtocolState, Role};

// ── ConnPhase ─────────────────────────────────────────────────────────────────

/// The application-level connection phase (above the SP9.1 wire handshake).
///
/// ```rust
/// use reovim_server_rt::conn::ConnPhase;
///
/// assert_ne!(ConnPhase::Handshake, ConnPhase::Ready);
/// assert_ne!(ConnPhase::Ready, ConnPhase::Attached);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnPhase {
    /// SP9.1 handshake not yet complete.
    Handshake,
    /// Handshake complete; waiting for `Attach`.
    Ready,
    /// `Attach` received and acknowledged; notify stream active (SP12).
    Attached,
}

// ── ConnState ─────────────────────────────────────────────────────────────────

/// Per-connection wire-protocol + application state.
///
/// Holds the `ProtocolState` machine (SP9.1 + correlation + unknown-tag rules)
/// and the application `ConnPhase`. Together they determine the action to take
/// on each incoming frame.
///
/// # Examples
///
/// ```rust
/// use reovim_server_rt::conn::{ConnPhase, ConnState};
///
/// let cs = ConnState::new();
/// assert_eq!(cs.phase(), ConnPhase::Handshake);
/// assert!(!cs.is_attached());
/// ```
pub struct ConnState {
    /// SP9.1 + SP11 + §10.3 state machine (from `uapi/protocol`).
    pub protocol: ProtocolState,
    /// Application-level attach phase.
    phase: ConnPhase,
}

impl ConnState {
    /// Creates a fresh `ConnState` in the `Handshake` phase.
    ///
    /// ```rust
    /// use reovim_server_rt::conn::{ConnPhase, ConnState};
    ///
    /// let cs = ConnState::new();
    /// assert_eq!(cs.phase(), ConnPhase::Handshake);
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            protocol: ProtocolState::new(Role::Server),
            phase: ConnPhase::Handshake,
        }
    }

    /// Returns the current application-level phase.
    ///
    /// ```rust
    /// use reovim_server_rt::conn::{ConnPhase, ConnState};
    ///
    /// assert_eq!(ConnState::new().phase(), ConnPhase::Handshake);
    /// ```
    #[must_use]
    pub const fn phase(&self) -> ConnPhase {
        self.phase
    }

    /// Returns `true` when `Attach` has been acknowledged (SP12 notify active).
    ///
    /// ```rust
    /// use reovim_server_rt::conn::ConnState;
    ///
    /// assert!(!ConnState::new().is_attached());
    /// ```
    #[must_use]
    pub const fn is_attached(&self) -> bool {
        matches!(self.phase, ConnPhase::Attached)
    }

    /// Advances from `Handshake` to `Ready` after the SP9.1 `Hello`/`HelloAck`
    /// exchange. No-op if already `Ready` or `Attached`.
    ///
    /// ```rust
    /// use reovim_server_rt::conn::{ConnPhase, ConnState};
    ///
    /// let mut cs = ConnState::new();
    /// cs.set_ready();
    /// assert_eq!(cs.phase(), ConnPhase::Ready);
    /// ```
    pub fn set_ready(&mut self) {
        if self.phase == ConnPhase::Handshake {
            self.phase = ConnPhase::Ready;
        }
    }

    /// Advances from `Ready` to `Attached` after a successful `Attach`.
    /// Returns `false` if the connection is already `Attached` (SP1 — second
    /// `Attach` is a conflict).
    ///
    /// ```rust
    /// use reovim_server_rt::conn::{ConnPhase, ConnState};
    ///
    /// let mut cs = ConnState::new();
    /// cs.set_ready();
    /// assert!(cs.set_attached());
    /// assert_eq!(cs.phase(), ConnPhase::Attached);
    /// // Second attach returns false (SP1).
    /// assert!(!cs.set_attached());
    /// ```
    pub fn set_attached(&mut self) -> bool {
        if self.phase == ConnPhase::Ready {
            self.phase = ConnPhase::Attached;
            true
        } else {
            false
        }
    }
}

impl Default for ConnState {
    fn default() -> Self {
        Self::new()
    }
}

// L12 layout: tests in sibling conn_tests.rs, declared in lib.rs.
