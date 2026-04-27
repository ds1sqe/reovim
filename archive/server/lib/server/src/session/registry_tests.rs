use super::*;

#[test]
fn test_registry_insert_get() {
    let registry = SessionRegistry::new();
    let session = Arc::new(Session::new(SessionId::new("test")));

    registry.insert(&session);

    let found = registry.get(&SessionId::new("test"));
    assert!(found.is_some());
}

#[test]
fn test_registry_next_client_id() {
    let registry = SessionRegistry::new();

    let id1 = registry.next_client_id();
    let id2 = registry.next_client_id();

    assert_ne!(id1, id2);
}

#[test]
fn test_registry_remove() {
    let registry = SessionRegistry::new();
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);

    let removed = registry.remove(&SessionId::new("test"));
    assert!(removed.is_some());
    assert!(registry.get(&SessionId::new("test")).is_none());
}

#[test]
fn test_registry_remove_nonexistent() {
    let registry = SessionRegistry::new();
    let removed = registry.remove(&SessionId::new("nonexistent"));
    assert!(removed.is_none());
}

#[test]
fn test_registry_list() {
    let registry = SessionRegistry::new();
    assert!(registry.list().is_empty());

    let session = Arc::new(Session::new(SessionId::new("alpha")));
    registry.insert(&session);

    let list = registry.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0], SessionId::new("alpha"));
}

#[test]
fn test_registry_len_and_is_empty() {
    let registry = SessionRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);

    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_registry_default() {
    let registry = SessionRegistry::default();
    assert!(registry.is_empty());
}

#[test]
fn test_registry_insert_replaces() {
    let registry = SessionRegistry::new();
    let s1 = Arc::new(Session::new(SessionId::new("test")));
    let s2 = Arc::new(Session::new(SessionId::new("test")));

    registry.insert(&s1);
    registry.insert(&s2);
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_registry_multiple_sessions() {
    let registry = SessionRegistry::new();

    let s1 = Arc::new(Session::new(SessionId::new("alpha")));
    let s2 = Arc::new(Session::new(SessionId::new("beta")));
    let s3 = Arc::new(Session::new(SessionId::new("gamma")));

    registry.insert(&s1);
    registry.insert(&s2);
    registry.insert(&s3);

    assert_eq!(registry.len(), 3);
    assert!(!registry.is_empty());

    // All sessions should be retrievable
    assert!(registry.get(&SessionId::new("alpha")).is_some());
    assert!(registry.get(&SessionId::new("beta")).is_some());
    assert!(registry.get(&SessionId::new("gamma")).is_some());
    assert!(registry.get(&SessionId::new("delta")).is_none());
}

#[test]
fn test_registry_next_client_id_monotonic() {
    let registry = SessionRegistry::new();

    let id1 = registry.next_client_id();
    let id2 = registry.next_client_id();
    let id3 = registry.next_client_id();

    // Should be strictly increasing
    assert!(id1.as_usize() < id2.as_usize());
    assert!(id2.as_usize() < id3.as_usize());

    // First ID should be 1 (not 0)
    assert_eq!(id1.as_usize(), 1);
}

#[test]
fn test_registry_list_multiple() {
    let registry = SessionRegistry::new();

    let s1 = Arc::new(Session::new(SessionId::new("x")));
    let s2 = Arc::new(Session::new(SessionId::new("y")));

    registry.insert(&s1);
    registry.insert(&s2);

    let list = registry.list();
    assert_eq!(list.len(), 2);
    assert!(list.contains(&SessionId::new("x")));
    assert!(list.contains(&SessionId::new("y")));
}

#[test]
fn test_registry_remove_then_reinsert() {
    let registry = SessionRegistry::new();
    let session = Arc::new(Session::new(SessionId::new("test")));

    registry.insert(&session);
    assert_eq!(registry.len(), 1);

    let removed = registry.remove(&SessionId::new("test"));
    assert!(removed.is_some());
    assert_eq!(registry.len(), 0);

    // Re-insert
    registry.insert(&session);
    assert_eq!(registry.len(), 1);
    assert!(registry.get(&SessionId::new("test")).is_some());
}

#[test]
fn test_registry_get_returns_none_for_nonexistent() {
    let registry = SessionRegistry::new();
    assert!(registry.get(&SessionId::new("nope")).is_none());
}
