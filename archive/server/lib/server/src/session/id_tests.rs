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
