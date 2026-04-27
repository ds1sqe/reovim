use {super::*, crate::session::Session};

#[test]
fn tick_key_equality() {
    let k1 = TickKey {
        client_id: DriverClientId::new(1),
        kind: "tetromino",
    };
    let k2 = TickKey {
        client_id: DriverClientId::new(1),
        kind: "tetromino",
    };
    assert_eq!(k1, k2);
}

#[test]
fn tick_key_different_client() {
    let k1 = TickKey {
        client_id: DriverClientId::new(1),
        kind: "tetromino",
    };
    let k2 = TickKey {
        client_id: DriverClientId::new(2),
        kind: "tetromino",
    };
    assert_ne!(k1, k2);
}

#[test]
fn tick_key_different_kind() {
    let k1 = TickKey {
        client_id: DriverClientId::new(1),
        kind: "tetromino",
    };
    let k2 = TickKey {
        client_id: DriverClientId::new(1),
        kind: "other",
    };
    assert_ne!(k1, k2);
}

#[test]
fn tick_key_debug() {
    let key = TickKey {
        client_id: DriverClientId::new(42),
        kind: "test",
    };
    let debug = format!("{key:?}");
    assert!(debug.contains("TickKey"));
    assert!(debug.contains("42"));
}

#[test]
fn to_server_id_converts() {
    let driver_id = DriverClientId::new(7);
    let server_id = to_server_id(driver_id);
    assert_eq!(server_id.as_usize(), 7);
}

#[test]
fn scheduler_new() {
    let sessions = Arc::new(SessionRegistry::new());
    let bridges = Arc::new(BridgeRegistry::default());
    let session_id = SessionId::new("test");
    let scheduler = TokioTickScheduler::new(sessions, session_id, bridges);
    assert!(scheduler.tasks.lock().is_empty());
}

#[test]
fn stop_noop_when_no_task() {
    let sessions = Arc::new(SessionRegistry::new());
    let bridges = Arc::new(BridgeRegistry::default());
    let session_id = SessionId::new("test");
    let scheduler = TokioTickScheduler::new(sessions, session_id, bridges);
    scheduler.stop(DriverClientId::new(1), "tetromino");
}

#[test]
fn pause_noop_when_no_task() {
    let sessions = Arc::new(SessionRegistry::new());
    let bridges = Arc::new(BridgeRegistry::default());
    let session_id = SessionId::new("test");
    let scheduler = TokioTickScheduler::new(sessions, session_id, bridges);
    scheduler.pause(DriverClientId::new(1), "tetromino");
}

#[test]
fn resume_noop_when_no_task() {
    let sessions = Arc::new(SessionRegistry::new());
    let bridges = Arc::new(BridgeRegistry::default());
    let session_id = SessionId::new("test");
    let scheduler = TokioTickScheduler::new(sessions, session_id, bridges);
    scheduler.resume(DriverClientId::new(1), "tetromino");
}

#[test]
fn start_with_no_session_is_noop() {
    let sessions = Arc::new(SessionRegistry::new());
    let bridges = Arc::new(BridgeRegistry::default());
    let session_id = SessionId::new("nonexistent");
    let scheduler = TokioTickScheduler::new(sessions, session_id, bridges);
    scheduler.start(DriverClientId::new(1), "tetromino", Duration::from_millis(50));
    assert!(scheduler.tasks.lock().is_empty());
}

#[tokio::test]
async fn start_stop_lifecycle() {
    let sessions = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    sessions.insert(&session);

    let bridges = Arc::new(BridgeRegistry::default());
    let session_id = SessionId::new("test");
    let scheduler =
        TokioTickScheduler::new(Arc::clone(&sessions), session_id, Arc::clone(&bridges));

    session.add_client(ClientId::new(1));

    scheduler.start(DriverClientId::new(1), "test", Duration::from_millis(10));
    assert_eq!(scheduler.tasks.lock().len(), 1);

    scheduler.stop(DriverClientId::new(1), "test");
    assert!(scheduler.tasks.lock().is_empty());
}

#[tokio::test]
async fn start_replaces_existing_task() {
    let sessions = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    sessions.insert(&session);
    session.add_client(ClientId::new(1));

    let bridges = Arc::new(BridgeRegistry::default());
    let session_id = SessionId::new("test");
    let scheduler =
        TokioTickScheduler::new(Arc::clone(&sessions), session_id, Arc::clone(&bridges));

    scheduler.start(DriverClientId::new(1), "test", Duration::from_millis(100));
    assert_eq!(scheduler.tasks.lock().len(), 1);

    // Start again should replace (not add second)
    scheduler.start(DriverClientId::new(1), "test", Duration::from_millis(50));
    assert_eq!(scheduler.tasks.lock().len(), 1);

    scheduler.stop(DriverClientId::new(1), "test");
}

#[tokio::test]
async fn pause_and_resume() {
    let sessions = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    sessions.insert(&session);
    session.add_client(ClientId::new(1));

    let bridges = Arc::new(BridgeRegistry::default());
    let session_id = SessionId::new("test");
    let scheduler =
        TokioTickScheduler::new(Arc::clone(&sessions), session_id, Arc::clone(&bridges));

    scheduler.start(DriverClientId::new(1), "test", Duration::from_millis(100));

    // Verify pause sets flag
    scheduler.pause(DriverClientId::new(1), "test");
    let key = TickKey {
        client_id: DriverClientId::new(1),
        kind: "test",
    };
    let is_paused = scheduler.tasks.lock()[&key].paused.load(Ordering::Relaxed);
    assert!(is_paused);

    // Verify resume clears flag
    scheduler.resume(DriverClientId::new(1), "test");
    let is_paused = scheduler.tasks.lock()[&key].paused.load(Ordering::Relaxed);
    assert!(!is_paused);

    scheduler.stop(DriverClientId::new(1), "test");
}
