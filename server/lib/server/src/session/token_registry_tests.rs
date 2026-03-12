use super::*;

#[test]
fn register_generates_unique_tokens() {
    let registry = TokenRegistry::new();
    let t1 = registry.register(ClientId::new(1));
    let t2 = registry.register(ClientId::new(2));
    assert_ne!(t1, t2);
    assert_eq!(t1.as_str().len(), 32); // 128-bit hex
}

#[test]
fn resolve_returns_correct_client() {
    let registry = TokenRegistry::new();
    let token = registry.register(ClientId::new(42));
    assert_eq!(registry.resolve(&token), Some(ClientId::new(42)));
}

#[test]
fn resolve_unknown_token_returns_none() {
    let registry = TokenRegistry::new();
    let fake = SessionToken::from("nonexistent");
    assert_eq!(registry.resolve(&fake), None);
}

#[test]
fn revoke_removes_mapping() {
    let registry = TokenRegistry::new();
    let token = registry.register(ClientId::new(1));

    let removed = registry.revoke(&token);
    assert_eq!(removed, Some(ClientId::new(1)));
    assert_eq!(registry.resolve(&token), None);
    assert!(registry.is_empty());
}

#[test]
fn revoke_by_client_removes_mapping() {
    let registry = TokenRegistry::new();
    let token = registry.register(ClientId::new(7));

    let removed = registry.revoke_by_client(ClientId::new(7));
    assert_eq!(removed, Some(token));
    assert!(registry.is_empty());
}

#[test]
fn revoke_by_client_idempotent() {
    let registry = TokenRegistry::new();
    registry.register(ClientId::new(1));

    assert!(registry.revoke_by_client(ClientId::new(1)).is_some());
    assert!(registry.revoke_by_client(ClientId::new(1)).is_none());
}

#[test]
fn register_replaces_existing_token() {
    let registry = TokenRegistry::new();
    let old_token = registry.register(ClientId::new(1));
    let new_token = registry.register(ClientId::new(1));

    // Old token should be invalidated
    assert_eq!(registry.resolve(&old_token), None);
    // New token should work
    assert_eq!(registry.resolve(&new_token), Some(ClientId::new(1)));
    // Only one entry
    assert_eq!(registry.len(), 1);
}

#[test]
fn len_and_is_empty() {
    let registry = TokenRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);

    registry.register(ClientId::new(1));
    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);

    registry.register(ClientId::new(2));
    assert_eq!(registry.len(), 2);
}

#[test]
fn session_token_display() {
    let token = SessionToken::from("test-token-123");
    assert_eq!(format!("{token}"), "test-token-123");
}

#[test]
fn default_creates_empty_registry() {
    let registry = TokenRegistry::default();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}
