//! Tokio-based tick scheduler (#546).
//!
//! Spawns periodic tasks that call `ExtensionStateBridge::tick()` for
//! server-driven state advancement (e.g., game gravity without key input).
//!
//! # Architecture
//!
//! ```text
//! TickScheduler trait (driver)        → WHAT a scheduler does
//! TokioTickScheduler (this module)    → HOW (tokio::spawn + interval)
//! Command handlers (module)           → WHEN to start/stop/pause/resume
//! ```

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use {
    parking_lot::Mutex,
    reovim_driver_session::{
        ClientId as DriverClientId, bridges::BridgeRegistry, tick::TickScheduler,
    },
};

use crate::session::{ClientId, SessionId, SessionRegistry};

/// Convert driver-layer `ClientId` to server-layer `ClientId`.
const fn to_server_id(id: DriverClientId) -> ClientId {
    ClientId::new(id.as_usize())
}

/// Handle for a running tick task.
struct TickHandle {
    /// Tokio abort handle for cancellation.
    abort_handle: tokio::task::AbortHandle,
    /// Pause flag checked each iteration.
    paused: Arc<AtomicBool>,
}

/// Key for looking up tick handles.
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
struct TickKey {
    client_id: DriverClientId,
    kind: &'static str,
}

/// Tokio-based tick scheduler.
///
/// Spawns `tokio::spawn` tasks that call `ExtensionStateBridge::tick()` at
/// regular intervals. Each task holds `Arc<Session>` and `Arc<BridgeRegistry>`.
pub struct TokioTickScheduler {
    /// Active tick tasks.
    tasks: Mutex<HashMap<TickKey, TickHandle>>,
    /// Session registry for resolving sessions.
    sessions: Arc<SessionRegistry>,
    /// Default session ID.
    session_id: SessionId,
    /// Bridge registry for tick + notification.
    bridges: Arc<BridgeRegistry>,
}

impl TokioTickScheduler {
    /// Create a new scheduler.
    #[must_use]
    pub fn new(
        sessions: Arc<SessionRegistry>,
        session_id: SessionId,
        bridges: Arc<BridgeRegistry>,
    ) -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
            sessions,
            session_id,
            bridges,
        }
    }
}

impl TickScheduler for TokioTickScheduler {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn start(&self, client_id: DriverClientId, kind: &'static str, interval: Duration) {
        let key = TickKey { client_id, kind };

        // Stop existing task if any
        {
            let mut tasks = self.tasks.lock();
            if let Some(handle) = tasks.remove(&key) {
                handle.abort_handle.abort();
            }
        }

        let paused = Arc::new(AtomicBool::new(false));
        let paused_clone = Arc::clone(&paused);

        let Some(session) = self.sessions.get(&self.session_id) else {
            return;
        };
        let bridges = Arc::clone(&self.bridges);
        let server_client_id = to_server_id(client_id);

        let join_handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                ticker.tick().await;

                if paused_clone.load(Ordering::Relaxed) {
                    continue;
                }

                let Some(bridge) = bridges.get(kind) else {
                    break;
                };

                // Phase 1: Mutate under write locks
                let changed =
                    session.with_tick_mut(server_client_id, |client_ext, shared_ext, services| {
                        bridge.tick(client_ext, shared_ext, services)
                    });

                // Phase 2: Emit notification under read locks (if changed)
                if changed == Some(true) {
                    use crate::grpc::notification_builder;

                    #[allow(clippy::cast_possible_truncation)]
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis() as u64);

                    if let Some(notif) = notification_builder::build_extension_notification(
                        kind,
                        &session,
                        timestamp,
                        client_id.as_usize() as u64,
                        &bridges,
                    ) {
                        session.emit_notification(notif);
                    }
                }
            }
        });

        let handle = TickHandle {
            abort_handle: join_handle.abort_handle(),
            paused,
        };

        self.tasks.lock().insert(key, handle);
    }

    fn stop(&self, client_id: DriverClientId, kind: &'static str) {
        let key = TickKey { client_id, kind };
        let removed = self.tasks.lock().remove(&key);
        if let Some(handle) = removed {
            handle.abort_handle.abort();
        }
    }

    fn pause(&self, client_id: DriverClientId, kind: &'static str) {
        let key = TickKey { client_id, kind };
        let tasks = self.tasks.lock();
        if let Some(handle) = tasks.get(&key) {
            handle.paused.store(true, Ordering::Relaxed);
        }
    }

    fn resume(&self, client_id: DriverClientId, kind: &'static str) {
        let key = TickKey { client_id, kind };
        let tasks = self.tasks.lock();
        if let Some(handle) = tasks.get(&key) {
            handle.paused.store(false, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
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
}
