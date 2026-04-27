use {super::*, crate::wire::SyncMode};

fn test_presence(id: &str) -> ClientPresence {
    ClientPresence::new(id)
}

#[test]
fn test_presence_tracker_new() {
    let tracker = PresenceTracker::new();
    assert!(tracker.is_empty());
    assert_eq!(tracker.len(), 0);
}

#[test]
fn test_presence_tracker_upsert() {
    let mut tracker = PresenceTracker::new();

    tracker.upsert(test_presence("client-1"));
    assert_eq!(tracker.len(), 1);
    assert!(tracker.contains("client-1"));

    // Update existing
    let mut updated = test_presence("client-1");
    updated.is_active = false;
    tracker.upsert(updated);
    assert_eq!(tracker.len(), 1);
    assert!(!tracker.get("client-1").unwrap().is_active);
}

#[test]
fn test_presence_tracker_remove() {
    let mut tracker = PresenceTracker::new();
    tracker.upsert(test_presence("client-1"));

    let removed = tracker.remove("client-1");
    assert!(removed.is_some());
    assert!(tracker.is_empty());

    let removed = tracker.remove("nonexistent");
    assert!(removed.is_none());
}

#[test]
fn test_presence_tracker_get() {
    let mut tracker = PresenceTracker::new();
    tracker.upsert(test_presence("client-1"));

    assert!(tracker.get("client-1").is_some());
    assert!(tracker.get("nonexistent").is_none());
}

#[test]
fn test_presence_tracker_client_ids() {
    let mut tracker = PresenceTracker::new();
    tracker.upsert(test_presence("a"));
    tracker.upsert(test_presence("b"));
    tracker.upsert(test_presence("c"));

    let ids: Vec<_> = tracker.client_ids().collect();
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"b"));
    assert!(ids.contains(&"c"));
}

#[test]
fn test_presence_tracker_clients_in_buffer() {
    let mut tracker = PresenceTracker::new();
    tracker.upsert(test_presence("a").with_position(1, 0, 0));
    tracker.upsert(test_presence("b").with_position(2, 0, 0));
    tracker.upsert(test_presence("c").with_position(1, 10, 5));

    assert_eq!(tracker.clients_in_buffer(1).count(), 2);
}

#[test]
fn test_presence_tracker_active_clients() {
    let mut tracker = PresenceTracker::new();
    tracker.upsert(test_presence("active-1"));
    tracker.upsert(test_presence("active-2"));

    let mut inactive = test_presence("inactive");
    inactive.is_active = false;
    tracker.upsert(inactive);

    assert_eq!(tracker.active_clients().count(), 2);
}

#[test]
fn test_presence_tracker_broadcasters() {
    let mut tracker = PresenceTracker::new();
    tracker.upsert(test_presence("normal"));
    tracker.upsert(test_presence("broadcaster").with_sync_mode(SyncMode::Broadcast));

    let broadcasters: Vec<_> = tracker.broadcasters().collect();
    assert_eq!(broadcasters.len(), 1);
    assert_eq!(broadcasters[0].client_id, "broadcaster");
}

#[test]
fn test_presence_tracker_get_mut() {
    let mut tracker = PresenceTracker::new();
    tracker.upsert(test_presence("client-1"));

    let entry = tracker.get_mut("client-1");
    assert!(entry.is_some());
    entry.unwrap().is_active = false;
    assert!(!tracker.get("client-1").unwrap().is_active);

    assert!(tracker.get_mut("nonexistent").is_none());
}

#[test]
fn test_presence_tracker_all() {
    let mut tracker = PresenceTracker::new();
    tracker.upsert(test_presence("a"));
    tracker.upsert(test_presence("b"));

    assert_eq!(tracker.all().count(), 2);
}

#[test]
fn test_presence_tracker_clear() {
    let mut tracker = PresenceTracker::new();
    tracker.upsert(test_presence("a"));
    tracker.upsert(test_presence("b"));
    assert_eq!(tracker.len(), 2);

    tracker.clear();
    assert!(tracker.is_empty());
}
