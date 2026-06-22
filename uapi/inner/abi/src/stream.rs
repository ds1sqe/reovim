//! 6.3 §8 — Stream substrate types.

/// Opaque stream identifier (6.3 §8).
///
/// ```rust
/// use reovim_uapi_abi::stream::StreamId;
///
/// let id = StreamId(42);
/// assert_eq!(id.0, 42);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId(pub u64);

/// A snapshot of a stream handle's observable state (6.3 §8).
///
/// ```rust
/// use reovim_uapi_abi::stream::{StreamHandleInfo, StreamId, StreamState};
///
/// let info = StreamHandleInfo {
///     id:               StreamId(1),
///     state:            StreamState::Running,
///     bytes_in_flight:  0,
///     bytes_buffered:   0,
/// };
/// assert_eq!(info.state, StreamState::Running);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StreamHandleInfo {
    /// Stream identifier.
    pub id: StreamId,
    /// Current lifecycle state.
    pub state: StreamState,
    /// Bytes sent but not yet acknowledged.
    pub bytes_in_flight: u64,
    /// Bytes queued locally, not yet sent.
    pub bytes_buffered: u64,
}

/// Stream lifecycle state (6.3 §8).
///
/// ```rust
/// use reovim_uapi_abi::stream::StreamState;
///
/// assert_eq!(StreamState::Init as u8, 0);
/// assert_eq!(StreamState::Closed as u8, 5);
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Allocated but not yet started.
    Init = 0,
    /// Flowing normally.
    Running = 1,
    /// Blocked on receiver backpressure.
    BackpressureBlocked = 2,
    /// Data is stale; stream should be torn down.
    Stale = 3,
    /// Draining remaining in-flight data.
    Draining = 4,
    /// Fully closed.
    Closed = 5,
}
