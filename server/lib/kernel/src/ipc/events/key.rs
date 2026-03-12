//! Key press event for event-driven key dispatch.
//!
//! This event bridges the input driver and the runner's key handling
//! system, enabling loosely-coupled key processing via `EventBus`.
//!
//! # Design Philosophy
//!
//! Following "mechanism, not policy":
//! - `KeyPressEvent` is a pure data carrier (mechanism)
//! - How keys are resolved is decided by handlers (policy in runner)
//! - Session/client routing enables multi-session support
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::events::{KeyInput, KeyCode, Modifiers};
//! use reovim_kernel::api::v1::events::key::{KeyPressEvent, SessionId, ClientId};
//!
//! let key = KeyInput {
//!     key: KeyCode::Char('j'),
//!     modifiers: Modifiers::NONE,
//! };
//!
//! let event = KeyPressEvent::new(key, SessionId::new(0), ClientId::new(1));
//! assert_eq!(event.session_id.as_usize(), 0);
//! assert_eq!(event.client_id.as_usize(), 1);
//! ```

use crate::ipc::Event;

use super::KeyInput;

// =============================================================================
// Session and Client Identifiers
// =============================================================================

/// Session identifier for event routing.
///
/// Represents a named editing session (like tmux sessions). Multiple clients
/// can attach to the same session and share state.
///
/// This is a kernel-level mechanism type. The runner maps this to its
/// internal session management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(usize);

impl SessionId {
    /// Create a new session ID.
    #[must_use]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

/// Client identifier for event routing.
///
/// Represents a connection to the server (like a tmux client). Each client
/// has a unique ID within the server.
///
/// This is a kernel-level mechanism type. The runner maps this to its
/// internal client/connection management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(usize);

impl ClientId {
    /// Create a new client ID.
    #[must_use]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client-{}", self.0)
    }
}

// =============================================================================
// Key Press Event
// =============================================================================

/// Key press event emitted when a key is received.
///
/// This event is emitted by the runner's event loop and handled by
/// registered key handlers. The handler performs key resolution
/// and returns state changes via the result holder.
///
/// # Synchronous Processing
///
/// Key events are processed synchronously via `bus.emit()` because:
/// - Key handling must complete before the next key is read
/// - State mutations need to happen in order
/// - Latency requirements demand immediate processing
///
/// # Session Routing
///
/// The `session_id` and `client_id` identify which session should
/// process the key. Handlers filter events by session ID.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{EventBus, EventResult};
/// use reovim_kernel::api::v1::events::{KeyInput, KeyCode, Modifiers, priority};
/// use reovim_kernel::api::v1::events::key::{KeyPressEvent, SessionId, ClientId};
///
/// let bus = EventBus::new();
///
/// // Register handler for session 0
/// let session_id = SessionId::new(0);
/// let _sub = bus.subscribe::<KeyPressEvent, _>(priority::CORE, move |event| {
///     if event.session_id != session_id {
///         return EventResult::NotHandled;
///     }
///     // Process key...
///     EventResult::Handled
/// });
///
/// // Emit key event
/// let key = KeyInput {
///     key: KeyCode::Char('j'),
///     modifiers: Modifiers::NONE,
/// };
/// bus.emit(KeyPressEvent::new(key, SessionId::new(0), ClientId::new(1)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPressEvent {
    /// The key input from the input driver.
    pub key: KeyInput,
    /// Session ID for routing.
    pub session_id: SessionId,
    /// Client ID that originated this key.
    pub client_id: ClientId,
}

impl KeyPressEvent {
    /// Create a new key press event.
    #[must_use]
    pub const fn new(key: KeyInput, session_id: SessionId, client_id: ClientId) -> Self {
        Self {
            key,
            session_id,
            client_id,
        }
    }

    /// Get the event type name for logging.
    #[must_use]
    pub const fn event_type(&self) -> &'static str {
        "KeyPressEvent"
    }
}

impl Event for KeyPressEvent {}

// =============================================================================
