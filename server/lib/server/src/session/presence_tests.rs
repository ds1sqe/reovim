use super::*;

fn make_presence(id: usize, name: &str) -> ClientPresence {
    ClientPresence::new(ClientId::new(id), "tui", name)
}

#[test]
fn test_presence_map_new() {
    let map = PresenceMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn test_join_returns_empty_peers_first() {
    let map = PresenceMap::new();
    let presence = make_presence(1, "laptop");

    let peers = map.join(presence);

    assert!(peers.is_empty());
    assert_eq!(map.len(), 1);
}

#[test]
fn test_join_returns_existing_peers() {
    let map = PresenceMap::new();

    map.join(make_presence(1, "laptop"));
    let peers = map.join(make_presence(2, "phone"));

    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].client_id, ClientId::new(1));
    assert_eq!(map.len(), 2);
}

#[test]
fn test_join_multiple_clients() {
    let map = PresenceMap::new();

    map.join(make_presence(1, "laptop"));
    map.join(make_presence(2, "phone"));
    let peers = map.join(make_presence(3, "tablet"));

    assert_eq!(peers.len(), 2);
    assert_eq!(map.len(), 3);
}

#[test]
fn test_leave_removes_client() {
    let map = PresenceMap::new();
    let client_id = ClientId::new(1);

    map.join(make_presence(1, "laptop"));
    let removed = map.leave(client_id);

    assert!(removed.is_some());
    assert_eq!(removed.unwrap().display_name, "laptop");
    assert!(map.is_empty());
}

#[test]
fn test_leave_returns_none_for_unknown() {
    let map = PresenceMap::new();

    let removed = map.leave(ClientId::new(999));

    assert!(removed.is_none());
}

#[test]
fn test_update_modifies_presence() {
    let map = PresenceMap::new();
    let client_id = ClientId::new(1);

    map.join(make_presence(1, "laptop"));
    let updated = map.update(client_id, |p| {
        p.cursor = (10, 5);
        p.mode = "INSERT".to_string();
    });

    assert!(updated.is_some());
    let presence = updated.unwrap();
    assert_eq!(presence.cursor, (10, 5));
    assert_eq!(presence.mode, "INSERT");

    // Verify it was actually persisted
    let stored = map.get(client_id).unwrap();
    assert_eq!(stored.cursor, (10, 5));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_update_returns_none_for_unknown() {
    let map = PresenceMap::new();

    let updated = map.update(ClientId::new(999), |p| {
        p.cursor = (10, 5);
    });

    assert!(updated.is_none());
}

#[test]
fn test_get_returns_presence() {
    let map = PresenceMap::new();
    let client_id = ClientId::new(1);

    map.join(make_presence(1, "laptop"));
    let presence = map.get(client_id);

    assert!(presence.is_some());
    assert_eq!(presence.unwrap().display_name, "laptop");
}

#[test]
fn test_get_returns_none_for_unknown() {
    let map = PresenceMap::new();

    let presence = map.get(ClientId::new(999));

    assert!(presence.is_none());
}

#[test]
fn test_list_returns_all_clients() {
    let map = PresenceMap::new();

    map.join(make_presence(1, "laptop"));
    map.join(make_presence(2, "phone"));
    map.join(make_presence(3, "tablet"));

    let clients = map.list();
    assert_eq!(clients.len(), 3);
}

#[test]
fn test_followers_of_returns_following_clients() {
    let map = PresenceMap::new();
    let presenter_id = ClientId::new(1);
    let follower1_id = ClientId::new(2);
    let follower2_id = ClientId::new(3);

    // Set up presenter
    map.join(make_presence(1, "presenter"));
    map.update(presenter_id, |p| p.sync_mode = SyncMode::Present);

    // Set up followers
    map.join(make_presence(2, "follower1"));
    map.update(follower1_id, |p| {
        p.sync_mode = SyncMode::Follow {
            target: presenter_id,
        };
    });

    map.join(make_presence(3, "follower2"));
    map.update(follower2_id, |p| {
        p.sync_mode = SyncMode::Follow {
            target: presenter_id,
        };
    });

    // Independent client
    map.join(make_presence(4, "independent"));

    let followers = map.followers_of(presenter_id);

    assert_eq!(followers.len(), 2);
    assert!(followers.contains(&follower1_id));
    assert!(followers.contains(&follower2_id));
}

#[test]
fn test_followers_of_empty_when_no_followers() {
    let map = PresenceMap::new();

    map.join(make_presence(1, "client1"));
    map.join(make_presence(2, "client2"));

    let followers = map.followers_of(ClientId::new(1));

    assert!(followers.is_empty());
}

#[test]
fn test_contains() {
    let map = PresenceMap::new();
    let client_id = ClientId::new(1);

    assert!(!map.contains(client_id));

    map.join(make_presence(1, "laptop"));
    assert!(map.contains(client_id));

    map.leave(client_id);
    assert!(!map.contains(client_id));
}

#[test]
fn test_sync_mode_default() {
    let presence = make_presence(1, "laptop");
    assert_eq!(presence.sync_mode, SyncMode::Independent);
}

#[test]
fn test_client_presence_new() {
    let presence = ClientPresence::new(ClientId::new(42), "android", "phone");

    assert_eq!(presence.client_id, ClientId::new(42));
    assert_eq!(presence.client_type, "android");
    assert_eq!(presence.display_name, "phone");
    assert_eq!(presence.buffer_id, None);
    assert_eq!(presence.cursor, (0, 0));
    assert_eq!(presence.visible_lines, (0, 24));
    assert_eq!(presence.mode, "NORMAL");
    assert_eq!(presence.sync_mode, SyncMode::Independent);
}

#[test]
fn test_joined_at_ms() {
    let presence = make_presence(1, "laptop");
    let ms = presence.joined_at_ms();

    // Should be a reasonable timestamp (after year 2020)
    assert!(ms > 1_577_836_800_000); // Jan 1, 2020
}

#[test]
fn test_concurrent_join_leave() {
    use std::{sync::Arc, thread};

    let map = Arc::new(PresenceMap::new());
    let mut handles = Vec::new();

    // Spawn 10 threads that join and leave
    for i in 0..10 {
        let map_clone = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            let presence = make_presence(i, &format!("client{i}"));
            map_clone.join(presence);

            // Do some operations
            let _ = map_clone.get(ClientId::new(i));
            let _ = map_clone.update(ClientId::new(i), |p| p.cursor = (i, i));
            let _ = map_clone.list();

            // Leave
            map_clone.leave(ClientId::new(i));
        }));
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // All clients should have left
    assert!(map.is_empty());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_sync_mode_follow() {
    let presenter_id = ClientId::new(1);
    let mode = SyncMode::Follow {
        target: presenter_id,
    };

    if let SyncMode::Follow { target } = mode {
        assert_eq!(target, presenter_id);
    } else {
        panic!("Expected Follow mode");
    }
}

#[test]
fn test_sync_mode_present() {
    let mode = SyncMode::Present;
    assert_eq!(mode, SyncMode::Present);
}
