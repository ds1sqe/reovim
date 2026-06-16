//! Stream substrate contract shapes (§4.4, #798).
//!
//! Concrete stream schemes provide cdylib-owned vtables later. The contract
//! tier carries safe in-process state: scheme metadata, handle state,
//! backpressure counters, typed options bytes, and the S5 control-op
//! partitioning.

use reovim_lib_ds::Bytes;

use crate::{
    id::{BufferId, CdylibId, SessionId, StreamId},
    routing::DomainApiVersion,
};

/// Stream handle lifecycle state (§4.4 §1).
///
/// ```rust
/// use reovim_subsys_domain::stream::StreamState;
///
/// assert!(StreamState::Running.accepts_emit());
/// assert!(!StreamState::Closed.accepts_emit());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamState {
    /// Open is in progress.
    Init,
    /// Bytes may flow.
    Running,
    /// Kernel backpressure is currently blocking new bytes.
    BackpressureBlocked,
    /// Underlying source ended; buffered bytes may drain.
    Stale,
    /// Close/unload drain is in progress.
    Draining,
    /// Handle is closed.
    Closed,
}

impl StreamState {
    /// Returns whether the state may accept a scheme emit.
    ///
    /// ```rust
    /// use reovim_subsys_domain::stream::StreamState;
    ///
    /// assert!(StreamState::BackpressureBlocked.accepts_emit());
    /// assert!(!StreamState::Draining.accepts_emit());
    /// ```
    #[must_use]
    pub const fn accepts_emit(self) -> bool {
        matches!(self, Self::Running | Self::BackpressureBlocked)
    }

    /// Returns whether the stream is terminally closed.
    ///
    /// ```rust
    /// use reovim_subsys_domain::stream::StreamState;
    ///
    /// assert!(StreamState::Closed.is_closed());
    /// ```
    #[must_use]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}

/// Backpressure counters tracked per stream (§4.4 §4).
///
/// ```rust
/// use reovim_subsys_domain::stream::{BackpressureBlock, BackpressureCounters};
///
/// let counters = BackpressureCounters::new(10, 2, 0);
/// assert_eq!(counters.blocked_by(8, 99), Some(BackpressureBlock::InFlight));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackpressureCounters {
    /// Bytes accepted by `emit` but not yet applied.
    pub bytes_in_flight: u64,
    /// Kernel-owned queued bytes.
    pub bytes_buffered: u64,
    /// Monotonic last-drain time in milliseconds.
    pub last_drain_at_ms: u64,
}

impl BackpressureCounters {
    /// Builds counters.
    ///
    /// ```rust
    /// use reovim_subsys_domain::stream::BackpressureCounters;
    ///
    /// assert_eq!(BackpressureCounters::new(1, 2, 3).bytes_buffered, 2);
    /// ```
    #[must_use]
    pub const fn new(bytes_in_flight: u64, bytes_buffered: u64, last_drain_at_ms: u64) -> Self {
        Self {
            bytes_in_flight,
            bytes_buffered,
            last_drain_at_ms,
        }
    }

    /// Returns which counter breaches a configured cap, if any.
    ///
    /// ```rust
    /// use reovim_subsys_domain::stream::{BackpressureBlock, BackpressureCounters};
    ///
    /// let counters = BackpressureCounters::new(1, 20, 0);
    /// assert_eq!(counters.blocked_by(10, 16), Some(BackpressureBlock::Buffered));
    /// ```
    #[must_use]
    pub const fn blocked_by(
        self,
        max_bytes_in_flight: u64,
        max_bytes_buffered: u64,
    ) -> Option<BackpressureBlock> {
        if self.bytes_in_flight > max_bytes_in_flight {
            Some(BackpressureBlock::InFlight)
        } else if self.bytes_buffered > max_bytes_buffered {
            Some(BackpressureBlock::Buffered)
        } else {
            None
        }
    }
}

/// Backpressure cap that blocked a stream.
///
/// ```rust
/// use reovim_subsys_domain::stream::BackpressureBlock;
///
/// assert_ne!(BackpressureBlock::InFlight, BackpressureBlock::Buffered);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackpressureBlock {
    /// `bytes_in_flight` exceeded its cap.
    InFlight,
    /// `bytes_buffered` exceeded its cap.
    Buffered,
}

/// S5 control-op class.
///
/// ```rust
/// use reovim_subsys_domain::stream::{StreamControlClass, StreamControlOp};
///
/// assert_eq!(StreamControlOp::new(0).class(), StreamControlClass::Invalid);
/// assert_eq!(StreamControlOp::new(101).class(), StreamControlClass::SchemePrivate);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamControlClass {
    /// Op `0`, always rejected.
    Invalid,
    /// Kernel-defined, kind-agnostic protocol op (`1..=100`).
    Kernel,
    /// Scheme-private op (`101..=255`).
    SchemePrivate,
    /// Reserved for future catalog evolution.
    Reserved,
}

/// Raw S5 control op.
///
/// ```rust
/// use reovim_subsys_domain::stream::{StreamControlClass, StreamControlOp};
///
/// assert_eq!(StreamControlOp::new(1).class(), StreamControlClass::Kernel);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamControlOp(u32);

impl StreamControlOp {
    /// Kernel inspect op.
    pub const KERNEL_INSPECT: Self = Self(1);
    /// Kernel drain op.
    pub const KERNEL_DRAIN: Self = Self(2);

    /// Wraps a raw op.
    ///
    /// ```rust
    /// use reovim_subsys_domain::stream::StreamControlOp;
    ///
    /// assert_eq!(StreamControlOp::new(7).as_u32(), 7);
    /// ```
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw op.
    ///
    /// ```rust
    /// use reovim_subsys_domain::stream::StreamControlOp;
    ///
    /// assert_eq!(StreamControlOp::KERNEL_DRAIN.as_u32(), 2);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Classifies this op by the S5 partition.
    ///
    /// ```rust
    /// use reovim_subsys_domain::stream::{StreamControlClass, StreamControlOp};
    ///
    /// assert_eq!(StreamControlOp::new(255).class(), StreamControlClass::SchemePrivate);
    /// assert_eq!(StreamControlOp::new(256).class(), StreamControlClass::Reserved);
    /// ```
    #[must_use]
    pub const fn class(self) -> StreamControlClass {
        match self.0 {
            0 => StreamControlClass::Invalid,
            1..=100 => StreamControlClass::Kernel,
            101..=255 => StreamControlClass::SchemePrivate,
            _ => StreamControlClass::Reserved,
        }
    }
}

/// Stream-scheme metadata registered during participant init (§4.4 §3).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime for the scheme name.
/// ```
pub struct StreamScheme {
    /// Scheme name bytes, for example `pty`.
    pub name: Bytes,
    /// Owning cdylib.
    pub owner_cdylib_id: CdylibId,
    /// Scheme API version.
    pub api_version: DomainApiVersion,
}

impl StreamScheme {
    /// Builds stream-scheme metadata.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for the scheme name.
    /// ```
    #[must_use]
    pub const fn new(
        name: Bytes,
        owner_cdylib_id: CdylibId,
        api_version: DomainApiVersion,
    ) -> Self {
        Self {
            name,
            owner_cdylib_id,
            api_version,
        }
    }
}

/// Typed stream-open options bytes (§4.4 §11).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime for option bytes.
/// ```
pub struct StreamOpts {
    /// URL bytes routed by scheme name.
    pub url: Bytes,
    /// Typed scheme-specific payload bytes.
    pub scheme_opts: Bytes,
}

impl StreamOpts {
    /// Builds stream-open options.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for option bytes.
    /// ```
    #[must_use]
    pub const fn new(url: Bytes, scheme_opts: Bytes) -> Self {
        Self { url, scheme_opts }
    }
}

/// Kernel-side stream handle state (§4.4 §1).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime for the scheme name.
/// ```
pub struct StreamHandle {
    /// Stream id.
    pub id: StreamId,
    /// Scheme name bytes.
    pub scheme: Bytes,
    /// Current stream state.
    pub state: StreamState,
    /// Optional owning session.
    pub session_id: Option<SessionId>,
    /// Optional target buffer.
    pub buffer_id: Option<BufferId>,
    /// Backpressure counters.
    pub backpressure: BackpressureCounters,
}

impl StreamHandle {
    /// Builds a stream handle.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for the scheme name.
    /// ```
    #[must_use]
    pub const fn new(
        id: StreamId,
        scheme: Bytes,
        state: StreamState,
        session_id: Option<SessionId>,
        buffer_id: Option<BufferId>,
        backpressure: BackpressureCounters,
    ) -> Self {
        Self {
            id,
            scheme,
            state,
            session_id,
            buffer_id,
            backpressure,
        }
    }
}

// L12 layout: tests in sibling stream_tests.rs, declared in lib.rs.
