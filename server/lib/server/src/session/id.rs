//! Session and client identifier types.

use std::sync::Arc;

/// Unique identifier for a session.
///
/// Sessions are named editing contexts (like tmux sessions).
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
    fn test_client_id_new() {
        let id = ClientId::new(42);
        assert_eq!(id.as_usize(), 42);
    }

    #[test]
    fn test_session_id_display() {
        let id = SessionId::new("my-session");
        assert_eq!(format!("{id}"), "my-session");
    }

    #[test]
    fn test_session_id_from_str() {
        let id: SessionId = "from-str".into();
        assert_eq!(id.name(), "from-str");
    }

    #[test]
    fn test_session_id_from_string() {
        let id: SessionId = String::from("from-string").into();
        assert_eq!(id.name(), "from-string");
    }

    #[test]
    fn test_session_id_clone_eq() {
        let id1 = SessionId::new("test");
        let id2 = id1.clone();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_session_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SessionId::new("a"));
        set.insert(SessionId::new("b"));
        set.insert(SessionId::new("a")); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_client_id_display() {
        let id = ClientId::new(5);
        assert_eq!(format!("{id}"), "client-5");
    }

    #[test]
    fn test_client_id_clone_eq_hash() {
        use std::collections::HashSet;
        let id1 = ClientId::new(1);
        let id2 = id1;
        assert_eq!(id1, id2);

        let mut set = HashSet::new();
        set.insert(ClientId::new(1));
        set.insert(ClientId::new(2));
        set.insert(ClientId::new(1));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_session_id_default_session() {
        let id = SessionId::default_session();
        assert_eq!(id.name(), "default");
    }

    #[test]
    fn test_session_id_debug() {
        let id = SessionId::new("test");
        let debug = format!("{id:?}");
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_client_id_debug() {
        let id = ClientId::new(7);
        let debug = format!("{id:?}");
        assert!(debug.contains('7'));
    }
}
