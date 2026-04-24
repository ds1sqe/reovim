use {
    super::{ClientDebugServiceImpl, cstr_to_string, probe_to_response, schema_list},
    crate::client_debug_registry::{
        ClientDebugRegistry, DebugDriverHandle, DebugObserverHandle, RegistryError,
    },
    reovim_client_subsys_debug::{DebugError, DebugProbe},
    reovim_protocol::v3::{
        DebugDriveCommand, DebugObserveStart, DebugObserveStop, DebugSelect, DebugStreamClientMsg,
        DebugStreamServerMsg, client_debug_service_server::ClientDebugService,
        debug_stream_client_msg, debug_stream_server_msg,
    },
    std::sync::Arc,
    tokio::sync::mpsc,
    tokio_stream::wrappers::ReceiverStream,
    tonic::Status,
};

// ─────────────────────────────────────────────────────────────────────────────
// Stub driver (same shape as the registry tests' stub)
// ─────────────────────────────────────────────────────────────────────────────

struct StubDriver {
    probe: DebugProbe,
    frames: Vec<Vec<u8>>,
    fail_observe: bool,
    observe_panics: bool,
    drive_panics: bool,
    drive_response: Vec<u8>,
}

impl StubDriver {
    fn echo(name: &str) -> Self {
        Self {
            probe: DebugProbe::new(name, "stub echo", &["frames"], &["echo"]),
            frames: vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()],
            fail_observe: false,
            observe_panics: false,
            drive_panics: false,
            drive_response: b"OK".to_vec(),
        }
    }
}

struct StubObserver {
    frames: std::vec::IntoIter<Vec<u8>>,
}

impl DebugDriverHandle for StubDriver {
    fn probe(&self) -> DebugProbe {
        self.probe
    }

    fn observe<'a>(
        &'a mut self,
        _selector: &[u8],
    ) -> Result<Box<dyn DebugObserverHandle + Send + 'a>, RegistryError> {
        if self.observe_panics {
            return Err(RegistryError::DriverPanicked);
        }
        if self.fail_observe {
            return Err(RegistryError::DriverError(DebugError("obs-fail".into())));
        }
        Ok(Box::new(StubObserver {
            frames: self.frames.clone().into_iter(),
        }))
    }

    fn drive(&mut self, _command: &[u8]) -> Result<Vec<u8>, RegistryError> {
        if self.drive_panics {
            return Err(RegistryError::DriverPanicked);
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
// Helpers for driving the service trait directly (no network)
// ─────────────────────────────────────────────────────────────────────────────

/// Drive the production `run_stream` function with an in-memory
/// client stream. Returns the server-side messages collected from the
/// outbound channel.
async fn drive_run_stream(
    registry: Arc<ClientDebugRegistry>,
    inputs: Vec<Result<DebugStreamClientMsg, Status>>,
) -> Vec<Result<DebugStreamServerMsg, String>> {
    let (client_tx, client_rx) = mpsc::channel::<Result<DebugStreamClientMsg, Status>>(16);
    let (srv_tx, mut srv_rx) = mpsc::channel::<Result<DebugStreamServerMsg, Status>>(64);

    for msg in inputs {
        client_tx.send(msg).await.unwrap();
    }
    drop(client_tx); // signal end of inputs

    let client_stream = ReceiverStream::new(client_rx);
    super::run_stream(registry, client_stream, srv_tx).await;

    let mut out = Vec::new();
    while let Ok(msg) = srv_rx.try_recv() {
        out.push(match msg {
            Ok(m) => Ok(m),
            Err(status) => Err(status.message().to_owned()),
        });
    }
    out
}

async fn run_state_machine(
    registry: Arc<ClientDebugRegistry>,
    inputs: Vec<DebugStreamClientMsg>,
) -> Vec<Result<DebugStreamServerMsg, String>> {
    drive_run_stream(registry, inputs.into_iter().map(Ok).collect()).await
}

fn select(name: &str) -> DebugStreamClientMsg {
    DebugStreamClientMsg {
        content: Some(debug_stream_client_msg::Content::Select(DebugSelect {
            driver_name: name.into(),
        })),
    }
}

fn observe_start(schema: &str) -> DebugStreamClientMsg {
    DebugStreamClientMsg {
        content: Some(debug_stream_client_msg::Content::ObserveStart(DebugObserveStart {
            schema: schema.into(),
        })),
    }
}

fn observe_stop() -> DebugStreamClientMsg {
    DebugStreamClientMsg {
        content: Some(debug_stream_client_msg::Content::ObserveStop(DebugObserveStop {})),
    }
}

fn drive(schema: &str, body: Vec<u8>) -> DebugStreamClientMsg {
    DebugStreamClientMsg {
        content: Some(debug_stream_client_msg::Content::DriveCmd(DebugDriveCommand {
            schema: schema.into(),
            body,
        })),
    }
}

fn empty_msg() -> DebugStreamClientMsg {
    DebugStreamClientMsg { content: None }
}

fn registry_with(name: &str, driver: StubDriver) -> Arc<ClientDebugRegistry> {
    let r = ClientDebugRegistry::new();
    r.register(name, Box::new(driver));
    Arc::new(r)
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper-function unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn cstr_to_string_truncates_at_nul() {
    let buf = b"hello\0world";
    assert_eq!(cstr_to_string(buf), "hello");
}

#[test]
fn cstr_to_string_no_nul_uses_full_buffer() {
    let buf = b"hello";
    assert_eq!(cstr_to_string(buf), "hello");
}

#[test]
fn cstr_to_string_lossy_on_invalid_utf8() {
    let buf = &[0xff, 0xfe, 0];
    let out = cstr_to_string(buf);
    assert!(!out.is_empty());
}

#[test]
fn schema_list_caps_at_count() {
    let slots: [[u8; 8]; 2] = [*b"alpha\0\0\0", *b"beta\0\0\0\0"];
    let list = schema_list::<8>(1, &slots);
    assert_eq!(list, vec!["alpha".to_string()]);
}

#[test]
fn schema_list_zero_count_returns_empty() {
    let slots: [[u8; 4]; 1] = [[b'x', 0, 0, 0]];
    let list = schema_list::<4>(0, &slots);
    assert!(list.is_empty());
}

#[test]
fn schema_list_caps_at_slots_when_count_exceeds() {
    let slots: [[u8; 4]; 2] = [[b'a', 0, 0, 0], [b'b', 0, 0, 0]];
    let list = schema_list::<4>(99, &slots);
    assert_eq!(list, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn probe_to_response_fills_all_fields() {
    let probe = DebugProbe::new("d", "hi", &["o1", "o2"], &["drv1"]);
    let resp = probe_to_response("d", &probe);
    assert_eq!(resp.driver_name, "d");
    assert_eq!(resp.description, "hi");
    assert_eq!(resp.observe_schemas, vec!["o1".to_string(), "o2".to_string()]);
    assert_eq!(resp.drive_schemas, vec!["drv1".to_string()]);
}

// ─────────────────────────────────────────────────────────────────────────────
// State-machine tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn observe_start_before_select_errors() {
    let reg = registry_with("d", StubDriver::echo("d"));
    let out = run_state_machine(reg, vec![observe_start("frames")]).await;
    assert_eq!(out.len(), 1);
    let msg = out[0].as_ref().expect("server msg");
    let Some(debug_stream_server_msg::Content::Error(err)) = &msg.content else {
        panic!("expected Error, got {:?}", msg.content);
    };
    assert!(err.message.contains("not selected"));
}

#[tokio::test]
async fn drive_before_select_errors() {
    let reg = registry_with("d", StubDriver::echo("d"));
    let out = run_state_machine(reg, vec![drive("echo", b"hi".to_vec())]).await;
    assert_eq!(out.len(), 1);
    let msg = out[0].as_ref().expect("server msg");
    let Some(debug_stream_server_msg::Content::Error(err)) = &msg.content else {
        panic!("expected Error, got {:?}", msg.content);
    };
    assert!(err.message.contains("not selected"));
}

#[tokio::test]
async fn empty_content_yields_error() {
    let reg = registry_with("d", StubDriver::echo("d"));
    let out = run_state_machine(reg, vec![empty_msg()]).await;
    assert_eq!(out.len(), 1);
    let msg = out[0].as_ref().expect("server msg");
    let Some(debug_stream_server_msg::Content::Error(err)) = &msg.content else {
        panic!("expected Error");
    };
    assert!(err.message.contains("empty"));
}

#[tokio::test]
async fn select_returns_probe_response() {
    let reg = registry_with("d", StubDriver::echo("d"));
    let out = run_state_machine(reg, vec![select("d")]).await;
    assert_eq!(out.len(), 1);
    let msg = out[0].as_ref().expect("server msg");
    let Some(debug_stream_server_msg::Content::ProbeResp(resp)) = &msg.content else {
        panic!("expected ProbeResp");
    };
    assert_eq!(resp.driver_name, "d");
    assert_eq!(resp.observe_schemas, vec!["frames".to_string()]);
}

#[tokio::test]
async fn select_unknown_driver_errors() {
    let reg = Arc::new(ClientDebugRegistry::new());
    let out = run_state_machine(reg, vec![select("missing")]).await;
    assert_eq!(out.len(), 1);
    let msg = out[0].as_ref().expect("server msg");
    let Some(debug_stream_server_msg::Content::Error(err)) = &msg.content else {
        panic!("expected Error");
    };
    assert!(err.message.contains("unknown driver"));
}

#[tokio::test]
async fn select_then_observe_streams_frames_and_eos() {
    let reg = registry_with("d", StubDriver::echo("d"));
    let out = run_state_machine(reg, vec![select("d"), observe_start("frames")]).await;
    // Expect: probe_resp, 3 observe_frames.
    assert_eq!(out.len(), 4);
    let mut iter = out.into_iter();
    let first = iter.next().unwrap().unwrap();
    assert!(matches!(first.content, Some(debug_stream_server_msg::Content::ProbeResp(_))));
    for expected in [b"a".to_vec(), b"b".to_vec(), b"c".to_vec()] {
        let frame = iter.next().unwrap().unwrap();
        let Some(debug_stream_server_msg::Content::ObserveFrame(f)) = frame.content else {
            panic!("expected ObserveFrame");
        };
        assert_eq!(f.body, expected);
    }
}

#[tokio::test]
async fn select_then_drive_returns_response() {
    let reg = registry_with("d", StubDriver::echo("d"));
    let out = run_state_machine(reg, vec![select("d"), drive("echo", b"x".to_vec())]).await;
    assert_eq!(out.len(), 2);
    let drive_resp = out[1].as_ref().expect("drive resp");
    let Some(debug_stream_server_msg::Content::DriveResp(resp)) = &drive_resp.content else {
        panic!("expected DriveResp");
    };
    assert_eq!(resp.body, b"OK".to_vec());
}

#[tokio::test]
async fn observe_stop_halts_pump_cleanly() {
    let reg = registry_with("d", StubDriver::echo("d"));
    // Send observe_start then immediately observe_stop. The stop
    // cancels the pump; depending on scheduling some frames may or
    // may not land. At minimum, no Error and no stream termination
    // should happen.
    let out =
        run_state_machine(reg, vec![select("d"), observe_start("frames"), observe_stop()]).await;
    // ProbeResp is the first, rest are either frames or nothing.
    assert!(!out.is_empty());
    assert!(out.iter().all(Result::is_ok));
}

#[tokio::test]
async fn second_select_cancels_prior_observer() {
    let reg = Arc::new(ClientDebugRegistry::new());
    reg.register("a", Box::new(StubDriver::echo("a")));
    reg.register("b", Box::new(StubDriver::echo("b")));
    let out = run_state_machine(reg, vec![select("a"), observe_start("frames"), select("b")]).await;
    // Two ProbeResp at least; no Error.
    let probe_count = out
        .iter()
        .filter(|r| {
            r.as_ref().is_ok_and(|m| {
                matches!(m.content, Some(debug_stream_server_msg::Content::ProbeResp(_)))
            })
        })
        .count();
    assert_eq!(probe_count, 2);
}

#[tokio::test]
async fn observe_start_unknown_schema_errors_but_stream_stays_open() {
    let mut d = StubDriver::echo("d");
    d.fail_observe = true;
    let reg = registry_with("d", d);
    let out = run_state_machine(
        Arc::clone(&reg),
        vec![
            select("d"),
            observe_start("bogus"),
            drive("echo", b"ping".to_vec()),
        ],
    )
    .await;
    // Expect: ProbeResp, Error (from observe), DriveResp (proves
    // stream stayed open after the observe error).
    let has_error = out.iter().any(|r| {
        r.as_ref()
            .is_ok_and(|m| matches!(m.content, Some(debug_stream_server_msg::Content::Error(_))))
    });
    assert!(has_error);
    let has_drive_resp = out.iter().any(|r| {
        r.as_ref().is_ok_and(|m| {
            matches!(m.content, Some(debug_stream_server_msg::Content::DriveResp(_)))
        })
    });
    assert!(has_drive_resp);
}

#[tokio::test]
async fn drive_driver_panic_terminates_and_evicts() {
    let mut d = StubDriver::echo("d");
    d.drive_panics = true;
    let reg = registry_with("d", d);
    let out =
        run_state_machine(Arc::clone(&reg), vec![select("d"), drive("echo", b"x".to_vec())]).await;
    // Last output must be the terminal Status::internal error.
    let terminal = out
        .iter()
        .find_map(|r| r.as_ref().err().map(ToOwned::to_owned))
        .expect("expected terminal status");
    assert!(terminal.contains("panicked"));
    // Driver evicted.
    assert!(!reg.list_drivers().contains(&"d".to_string()));
}

#[tokio::test]
async fn observe_driver_panic_terminates_and_evicts() {
    let mut d = StubDriver::echo("d");
    d.observe_panics = true;
    let reg = registry_with("d", d);
    let out = run_state_machine(Arc::clone(&reg), vec![select("d"), observe_start("frames")]).await;
    let terminal = out
        .iter()
        .find_map(|r| r.as_ref().err().map(ToOwned::to_owned))
        .expect("expected terminal status");
    assert!(terminal.contains("panicked"));
    assert!(!reg.list_drivers().contains(&"d".to_string()));
}

// ─────────────────────────────────────────────────────────────────────────────
// Service-impl constructor test (type-check path)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn service_impl_new_stores_registry() {
    let reg = Arc::new(ClientDebugRegistry::new());
    let svc = ClientDebugServiceImpl::new(Arc::clone(&reg));
    // The only observable effect is that the service is constructible;
    // integration tests drive the streaming RPC end-to-end.
    let _: &ClientDebugServiceImpl = &svc;
    let _: &dyn ClientDebugService<DebugStreamStream = _> = &svc;
}

// ─────────────────────────────────────────────────────────────────────────────
// Production `run_stream` path coverage for the uncommon branches the
// integration tests don't cover.
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn select_observe_panic_terminates_and_evicts() {
    // A driver whose `observe` returns `DriverPanicked`. Forces the
    // observer_pump's early-construction panic path
    // (registry.observe → Err(DriverPanicked) inside the pump task).
    struct PanicObserveDriver;
    impl DebugDriverHandle for PanicObserveDriver {
        fn probe(&self) -> DebugProbe {
            DebugProbe::new("po", "", &["f"], &[])
        }
        fn observe<'a>(
            &'a mut self,
            _: &[u8],
        ) -> Result<Box<dyn DebugObserverHandle + Send + 'a>, RegistryError> {
            Err(RegistryError::DriverPanicked)
        }
        fn drive(&mut self, _: &[u8]) -> Result<Vec<u8>, RegistryError> {
            Ok(Vec::new())
        }
    }
    let reg = Arc::new(ClientDebugRegistry::new());
    reg.register("po", Box::new(PanicObserveDriver));
    let out =
        drive_run_stream(Arc::clone(&reg), vec![Ok(select("po")), Ok(observe_start("f"))]).await;
    let terminal = out
        .iter()
        .find_map(|r| r.as_ref().err().map(ToOwned::to_owned))
        .expect("expected terminal status from observer-construction panic");
    assert!(terminal.contains("panicked"));
    assert!(!reg.list_drivers().contains(&"po".to_string()));
}

#[tokio::test]
async fn drive_non_panic_error_keeps_stream_open() {
    struct DriveErrorDriver;
    impl DebugDriverHandle for DriveErrorDriver {
        fn probe(&self) -> DebugProbe {
            DebugProbe::new("de", "", &[], &["e"])
        }
        fn observe<'a>(
            &'a mut self,
            _: &[u8],
        ) -> Result<Box<dyn DebugObserverHandle + Send + 'a>, RegistryError> {
            unreachable!();
        }
        fn drive(&mut self, _: &[u8]) -> Result<Vec<u8>, RegistryError> {
            Err(RegistryError::DriverError(DebugError("drive-bad".into())))
        }
    }
    let reg = Arc::new(ClientDebugRegistry::new());
    reg.register("de", Box::new(DriveErrorDriver));
    let out = drive_run_stream(
        Arc::clone(&reg),
        vec![
            Ok(select("de")),
            Ok(drive("e", b"x".to_vec())),
            // Another select afterwards to prove the stream is still
            // accepting messages after the non-terminal error.
            Ok(select("de")),
        ],
    )
    .await;
    let error_count = out
        .iter()
        .filter(|r| {
            r.as_ref().is_ok_and(|m| {
                matches!(m.content, Some(debug_stream_server_msg::Content::Error(_)))
            })
        })
        .count();
    let probe_count = out
        .iter()
        .filter(|r| {
            r.as_ref().is_ok_and(|m| {
                matches!(m.content, Some(debug_stream_server_msg::Content::ProbeResp(_)))
            })
        })
        .count();
    assert_eq!(error_count, 1, "one non-terminal error for drive");
    assert_eq!(probe_count, 2, "both probes should succeed");
    // Registry retains the driver (not a panic).
    assert!(reg.list_drivers().contains(&"de".to_string()));
}

#[tokio::test]
async fn drive_panic_terminates_and_evicts() {
    // Drive-side panic in the main run_stream loop.
    struct PanicDriveDriver;
    impl DebugDriverHandle for PanicDriveDriver {
        fn probe(&self) -> DebugProbe {
            DebugProbe::new("pd", "", &[], &["e"])
        }
        fn observe<'a>(
            &'a mut self,
            _: &[u8],
        ) -> Result<Box<dyn DebugObserverHandle + Send + 'a>, RegistryError> {
            unreachable!();
        }
        fn drive(&mut self, _: &[u8]) -> Result<Vec<u8>, RegistryError> {
            Err(RegistryError::DriverPanicked)
        }
    }
    let reg = Arc::new(ClientDebugRegistry::new());
    reg.register("pd", Box::new(PanicDriveDriver));
    let out =
        drive_run_stream(Arc::clone(&reg), vec![Ok(select("pd")), Ok(drive("e", b"x".to_vec()))])
            .await;
    let terminal = out
        .iter()
        .find_map(|r| r.as_ref().err().map(ToOwned::to_owned))
        .expect("drive panic must terminate");
    assert!(terminal.contains("panicked"));
    assert!(!reg.list_drivers().contains(&"pd".to_string()));
}

#[tokio::test]
async fn transport_error_breaks_stream_loop() {
    let reg = registry_with("d", StubDriver::echo("d"));
    let out =
        drive_run_stream(reg, vec![Err(Status::internal("simulated transport failure"))]).await;
    assert!(out.is_empty(), "got {out:?}");
}

#[tokio::test]
async fn transport_error_after_select_cancels_active_observer() {
    let reg = registry_with("d", StubDriver::echo("d"));
    let out = drive_run_stream(
        reg,
        vec![
            Ok(select("d")),
            Ok(observe_start("frames")),
            Err(Status::aborted("transport reset")),
        ],
    )
    .await;
    let has_probe = out.iter().any(|r| {
        r.as_ref().is_ok_and(|m| {
            matches!(m.content, Some(debug_stream_server_msg::Content::ProbeResp(_)))
        })
    });
    assert!(has_probe);
}

#[tokio::test]
async fn pump_send_fail_on_outbound_close_exits_cleanly() {
    // A driver with many frames so the pump keeps running long enough
    // for us to drop the outbound receiver mid-stream.
    struct ManyFramesDriver;
    struct ManyFramesObserver(u32);
    impl DebugDriverHandle for ManyFramesDriver {
        fn probe(&self) -> DebugProbe {
            DebugProbe::new("mf", "", &["f"], &[])
        }
        fn observe<'a>(
            &'a mut self,
            _: &[u8],
        ) -> Result<Box<dyn DebugObserverHandle + Send + 'a>, RegistryError> {
            Ok(Box::new(ManyFramesObserver(100)))
        }
        fn drive(&mut self, _: &[u8]) -> Result<Vec<u8>, RegistryError> {
            unreachable!();
        }
    }
    impl DebugObserverHandle for ManyFramesObserver {
        fn next_frame(&mut self) -> Result<Option<Vec<u8>>, RegistryError> {
            if self.0 == 0 {
                return Ok(None);
            }
            self.0 -= 1;
            Ok(Some(vec![0_u8; 4]))
        }
    }
    let reg = Arc::new(ClientDebugRegistry::new());
    reg.register("mf", Box::new(ManyFramesDriver));

    let (client_tx, client_rx) = mpsc::channel::<Result<DebugStreamClientMsg, Status>>(4);
    client_tx.send(Ok(select("mf"))).await.unwrap();
    client_tx.send(Ok(observe_start("f"))).await.unwrap();
    drop(client_tx);

    let (srv_tx, mut srv_rx) = mpsc::channel::<Result<DebugStreamServerMsg, Status>>(2);
    let task =
        tokio::spawn(super::run_stream(Arc::clone(&reg), ReceiverStream::new(client_rx), srv_tx));

    // Drain probe + one frame, then drop srv_rx so the pump's next
    // send() returns SendError — exercising the outbound-closed exit
    // path in observer_pump.
    let _probe = srv_rx.recv().await;
    let _frame = srv_rx.recv().await;
    drop(srv_rx);

    // run_stream must complete within a small bound after outbound
    // closes. The pump detects SendError and returns; main loop also
    // exits when the client stream is already drained.
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .expect("run_stream terminates within 2s after outbound close")
        .expect("run_stream task joins cleanly");
}

#[tokio::test]
async fn pump_observer_non_terminal_error_keeps_stream_open() {
    // Driver whose next_frame returns a recoverable error mid-stream.
    // Exercises observer_pump's non-panic error branch.
    struct FlakyDriver;
    struct FlakyObserver {
        step: u32,
    }
    impl DebugDriverHandle for FlakyDriver {
        fn probe(&self) -> DebugProbe {
            DebugProbe::new("flaky", "", &["f"], &[])
        }
        fn observe<'a>(
            &'a mut self,
            _: &[u8],
        ) -> Result<Box<dyn DebugObserverHandle + Send + 'a>, RegistryError> {
            Ok(Box::new(FlakyObserver { step: 0 }))
        }
        fn drive(&mut self, _: &[u8]) -> Result<Vec<u8>, RegistryError> {
            Ok(Vec::new())
        }
    }
    impl DebugObserverHandle for FlakyObserver {
        fn next_frame(&mut self) -> Result<Option<Vec<u8>>, RegistryError> {
            self.step += 1;
            match self.step {
                1 => Ok(Some(b"ok-1".to_vec())),
                2 => Err(RegistryError::DriverError(DebugError("transient".into()))),
                _ => Ok(None),
            }
        }
    }
    let reg = Arc::new(ClientDebugRegistry::new());
    reg.register("flaky", Box::new(FlakyDriver));
    let out = drive_run_stream(reg, vec![Ok(select("flaky")), Ok(observe_start("f"))]).await;
    let saw_error = out.iter().any(|r| {
        r.as_ref()
            .is_ok_and(|m| matches!(m.content, Some(debug_stream_server_msg::Content::Error(_))))
    });
    let saw_frame = out.iter().any(|r| {
        r.as_ref().is_ok_and(|m| {
            matches!(m.content, Some(debug_stream_server_msg::Content::ObserveFrame(_)))
        })
    });
    assert!(saw_error);
    assert!(saw_frame);
}
