//! Session and client identifier types.
//!
//! Provides strongly-typed identifiers for sessions and clients to prevent
//! accidental mixing of IDs and enable clear ownership semantics.

use std::sync::Arc;

/// Unique identifier for a session.
///
/// Sessions are named editing contexts (like tmux sessions). Multiple clients
/// can attach to the same session and share editor state.
///
/// # Examples
///
/// ```ignore
/// use runner::session::SessionId;
///
/// let session = SessionId::new("default");
/// let project = SessionId::new("my-project");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(Arc<str>);

impl SessionId {
    /// Create a new session ID from a string.
    #[must_use]
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self(name.into())
    }

    /// Create the default session ID.
    #[must_use]
    pub fn default_session() -> Self {
        Self::new("default")
    }

    /// Get the session name as a string slice.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::default_session()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Unique identifier for a client connection.
///
/// Each client that connects to the server receives a unique ID, even if they
/// connect to the same session. This allows tracking individual connections.
///
/// # Examples
///
/// ```ignore
/// use runner::session::ClientId;
///
/// let client = ClientId::new(1);
/// assert_eq!(client.as_usize(), 1);
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_new() {
        let id = SessionId::new("test");
        assert_eq!(id.name(), "test");
    }

    #[test]
    fn test_session_id_default() {
        let id = SessionId::default();
        assert_eq!(id.name(), "default");
    }

    #[test]
    fn test_session_id_equality() {
        let a = SessionId::new("foo");
        let b = SessionId::new("foo");
        let c = SessionId::new("bar");

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_session_id_from_str() {
        let id: SessionId = "my-session".into();
        assert_eq!(id.name(), "my-session");
    }

    #[test]
    fn test_client_id_new() {
        let id = ClientId::new(42);
        assert_eq!(id.as_usize(), 42);
    }

    #[test]
    fn test_client_id_display() {
        let id = ClientId::new(123);
        assert_eq!(format!("{id}"), "client-123");
    }
}
