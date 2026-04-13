use {
    super::*,
    crate::session::{ClientPresence, SyncMode},
};

#[test]
fn test_presence_service_len_and_is_empty() {
    let service = PresenceService::new();
    assert!(service.is_empty());
    assert_eq!(service.len(), 0);

    service.join(ClientPresence::new(ClientId::new(1), "tui", "test"));
    assert!(!service.is_empty());
    assert_eq!(service.len(), 1);
}

#[test]
fn test_presence_service_followers_of() {
    let service = PresenceService::new();
    let target = ClientId::new(1);
    let follower = ClientId::new(2);

    service.join(ClientPresence::new(target, "tui", "target"));
    let mut follower_presence = ClientPresence::new(follower, "tui", "follower");
    follower_presence.sync_mode = SyncMode::Follow { target };
    service.join(follower_presence);

    let followers = service.followers_of(target);
    assert_eq!(followers.len(), 1);
    assert_eq!(followers[0], follower);
}
