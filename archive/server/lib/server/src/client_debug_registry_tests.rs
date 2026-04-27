use {
    super::{ClientDebugRegistry, DebugDriverHandle, DebugObserverHandle, RegistryError},
    reovim_client_subsys_debug::{DebugError, DebugProbe},
    std::sync::atomic::Ordering,
};

// ─────────────────────────────────────────────────────────────────────────────
// Stub driver — exercises the registry without the cdylib path
// ─────────────────────────────────────────────────────────────────────────────

struct StubDriver {
    probe: DebugProbe,
    frames: Vec<Vec<u8>>,
    drive_response: Vec<u8>,
    fail_observe: bool,
    fail_drive: bool,
    observer_dropped: Arc<AtomicBool>,
}

impl StubDriver {
    fn new(name: &str) -> (Self, Arc<AtomicBool>) {
        let observer_dropped = Arc::new(AtomicBool::new(false));
        let driver = Self {
            probe: DebugProbe::new(name, "stub", &["frames"], &["echo"]),
            frames: vec![b"f0".to_vec(), b"f1".to_vec(), b"f2".to_vec()],
            drive_response: b"echoed".to_vec(),
            fail_observe: false,
            fail_drive: false,
            observer_dropped: observer_dropped.clone(),
        };
        (driver, observer_dropped)
    }
}

struct StubObserver {
    frames: std::vec::IntoIter<Vec<u8>>,
    drop_flag: Arc<AtomicBool>,
}

impl Drop for StubObserver {
    fn drop(&mut self) {
        self.drop_flag.store(true, Ordering::SeqCst);
    }
}

impl DebugDriverHandle for StubDriver {
    fn probe(&self) -> DebugProbe {
        self.probe
    }

    fn observe<'a>(
        &'a mut self,
        _selector: &[u8],
    ) -> Result<Box<dyn DebugObserverHandle + Send + 'a>, RegistryError> {
        if self.fail_observe {
            return Err(RegistryError::DriverError(DebugError("no".into())));
        }
        Ok(Box::new(StubObserver {
            frames: self.frames.clone().into_iter(),
            drop_flag: self.observer_dropped.clone(),
        }))
    }

    fn drive(&mut self, _command: &[u8]) -> Result<Vec<u8>, RegistryError> {
        if self.fail_drive {
            return Err(RegistryError::DriverError(DebugError("drive-fail".into())));
        }
        Ok(self.drive_response.clone())
    }
}

impl DebugObserverHandle for StubObserver {
    fn next_frame(&mut self) -> Result<Option<Vec<u8>>, RegistryError> {
        Ok(self.frames.next())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::{Arc, atomic::AtomicBool};

fn registry_with_stub(name: &str) -> (ClientDebugRegistry, Arc<AtomicBool>) {
    let (driver, drop_flag) = StubDriver::new(name);
    let reg = ClientDebugRegistry::new();
    reg.register(name, Box::new(driver));
    (reg, drop_flag)
}

#[test]
fn new_registry_is_empty() {
    let reg = ClientDebugRegistry::new();
    assert!(reg.list_drivers().is_empty());
    assert!(reg.probe("missing").is_none());
}

#[test]
fn default_matches_new() {
    let reg = ClientDebugRegistry::default();
    assert!(reg.list_drivers().is_empty());
}

#[test]
fn register_and_list() {
    let (reg, _) = registry_with_stub("a");
    let (b, _) = StubDriver::new("b");
    reg.register("b", Box::new(b));
    let mut names = reg.list_drivers();
    names.sort();
    assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn register_replaces_existing() {
    let reg = ClientDebugRegistry::new();
    let (d1, _) = StubDriver::new("same");
    let (d2, _) = StubDriver::new("same");
    reg.register("same", Box::new(d1));
    reg.register("same", Box::new(d2));
    assert_eq!(reg.list_drivers(), vec!["same".to_string()]);
}

#[test]
fn probe_reads_metadata() {
    let (reg, _) = registry_with_stub("probed");
    let probe = reg
        .probe("probed")
        .expect("registered")
        .expect("no contention");
    assert_eq!(&probe.driver_name[..6], b"probed");
    assert_eq!(probe.observe_schemas_count, 1);
    assert_eq!(probe.drive_schemas_count, 1);
}

#[test]
fn probe_unknown_returns_none() {
    let reg = ClientDebugRegistry::new();
    assert!(reg.probe("missing").is_none());
}

#[tokio::test]
async fn probe_async_reads_metadata() {
    let (reg, _) = registry_with_stub("probed");
    let probe = reg.probe_async("probed").await.expect("probe ok");
    assert_eq!(&probe.driver_name[..6], b"probed");
}

#[tokio::test]
async fn probe_async_unknown_errors() {
    let reg = ClientDebugRegistry::new();
    let err = reg.probe_async("missing").await.expect_err("should error");
    assert!(matches!(err, RegistryError::UnknownDriver(_)));
}

#[tokio::test]
async fn drive_roundtrip() {
    let (reg, _) = registry_with_stub("d");
    let out = reg.drive("d", b"hello").await.expect("drive ok");
    assert_eq!(out, b"echoed".to_vec());
}

#[tokio::test]
async fn drive_unknown_driver() {
    let reg = ClientDebugRegistry::new();
    let err = reg.drive("nope", b"x").await.expect_err("should error");
    assert!(matches!(err, RegistryError::UnknownDriver(_)));
}

#[tokio::test]
async fn drive_driver_error() {
    let (driver, _) = StubDriver::new("x");
    let mut driver = driver;
    driver.fail_drive = true;
    let reg = ClientDebugRegistry::new();
    reg.register("x", Box::new(driver));
    let err = reg.drive("x", b"c").await.expect_err("should error");
    assert!(matches!(err, RegistryError::DriverError(_)));
}

#[tokio::test]
async fn observe_pumps_frames_to_eos() {
    let (reg, drop_flag) = registry_with_stub("o");
    let mut pump = reg.observe("o", b"frames").await.expect("observe ok");
    assert_eq!(pump.next_frame().unwrap(), Some(b"f0".to_vec()));
    assert_eq!(pump.next_frame().unwrap(), Some(b"f1".to_vec()));
    assert_eq!(pump.next_frame().unwrap(), Some(b"f2".to_vec()));
    assert_eq!(pump.next_frame().unwrap(), None);
    drop(pump);
    assert!(drop_flag.load(Ordering::SeqCst));
}

#[tokio::test]
async fn observe_drop_releases_lock() {
    let (reg, _) = registry_with_stub("o");
    let pump = reg.observe("o", b"frames").await.expect("first observe ok");
    drop(pump);
    // Second observe must succeed — if the lock was still held this
    // would hang and the tokio::test would time out.
    let _second = reg
        .observe("o", b"frames")
        .await
        .expect("second observe ok");
}

#[tokio::test]
async fn observe_unknown_driver() {
    let reg = ClientDebugRegistry::new();
    let err = reg
        .observe("missing", b"frames")
        .await
        .expect_err("should error");
    assert!(matches!(err, RegistryError::UnknownDriver(_)));
}

#[tokio::test]
async fn observe_driver_error() {
    let (mut driver, _) = StubDriver::new("x");
    driver.fail_observe = true;
    let reg = ClientDebugRegistry::new();
    reg.register("x", Box::new(driver));
    let err = reg.observe("x", b"frames").await.expect_err("should error");
    assert!(matches!(err, RegistryError::DriverError(_)));
}

#[test]
fn evict_removes_driver() {
    let (reg, _) = registry_with_stub("gone");
    assert!(!reg.list_drivers().is_empty());
    reg.evict_on_panic("gone");
    assert!(reg.list_drivers().is_empty());
}

#[test]
fn evict_unknown_is_noop() {
    let reg = ClientDebugRegistry::new();
    reg.evict_on_panic("not-there");
    assert!(reg.list_drivers().is_empty());
}

#[test]
fn load_error_conversion() {
    use reovim_client_subsys_driver_loader::{LoadError, ValidationError};
    let err: RegistryError = LoadError::DriverError("bad".into()).into();
    assert!(matches!(err, RegistryError::DriverError(DebugError(ref s)) if s == "bad"));

    let err: RegistryError = LoadError::DriverPanicked.into();
    assert!(matches!(err, RegistryError::DriverPanicked));

    let err: RegistryError = LoadError::Validation(ValidationError::VtablePointerNull).into();
    assert!(matches!(err, RegistryError::Loader(_)));

    let err: RegistryError = LoadError::LibraryOpen("missing.so".into()).into();
    assert!(matches!(err, RegistryError::Loader(_)));
}

#[test]
fn registry_error_display() {
    let e = RegistryError::UnknownDriver("x".into());
    assert!(format!("{e}").contains('x'));
    let e = RegistryError::DriverError(DebugError("oops".into()));
    assert!(format!("{e}").contains("oops"));
    let e = RegistryError::DriverPanicked;
    assert!(format!("{e}").contains("panic"));
    let e = RegistryError::Loader("lib".into());
    assert!(format!("{e}").contains("lib"));
}

#[test]
fn registry_debug_format() {
    let (reg, _) = registry_with_stub("dbg");
    let out = format!("{reg:?}");
    assert!(out.contains("dbg"));
}

#[tokio::test]
async fn observer_pump_debug_format() {
    let (reg, _) = registry_with_stub("p");
    let pump = reg.observe("p", b"frames").await.expect("observe");
    let out = format!("{pump:?}");
    drop(pump);
    assert!(out.contains("ObserverPump"));
}

#[tokio::test]
async fn probe_under_contention_returns_loader_error() {
    let (reg, _) = registry_with_stub("busy");
    let _guard = reg
        .observe("busy", b"frames")
        .await
        .expect("take lock via observer");
    // Now probe sees contended try_lock.
    let result = reg.probe("busy").expect("registered");
    assert!(matches!(result, Err(RegistryError::Loader(_))));
}
