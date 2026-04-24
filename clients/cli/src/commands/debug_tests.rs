use {
    super::{
        consume_drive_response_stream, consume_observe_response_stream,
        consume_probe_response_stream, hex_encode, render_drive, render_frames, render_probe,
        resolve_input,
    },
    crate::OutputFormat,
    reovim_protocol::v3::{
        DebugDriveResponse, DebugObserveFrame, DebugProbeResponse, DebugStreamError,
        DebugStreamServerMsg, debug_stream_server_msg,
    },
    tokio::sync::mpsc,
    tokio_stream::wrappers::ReceiverStream,
    tonic::Status,
};

// ─────────────────────────────────────────────────────────────────────────────
// Builders for synthetic server-stream messages
// ─────────────────────────────────────────────────────────────────────────────

fn probe_resp(name: &str, obs: &[&str], drv: &[&str]) -> DebugStreamServerMsg {
    DebugStreamServerMsg {
        content: Some(debug_stream_server_msg::Content::ProbeResp(DebugProbeResponse {
            driver_name: name.into(),
            description: "test".into(),
            observe_schemas: obs.iter().map(|s| (*s).to_string()).collect(),
            drive_schemas: drv.iter().map(|s| (*s).to_string()).collect(),
        })),
    }
}

fn observe_frame(body: &[u8]) -> DebugStreamServerMsg {
    DebugStreamServerMsg {
        content: Some(debug_stream_server_msg::Content::ObserveFrame(DebugObserveFrame {
            body: body.to_vec(),
        })),
    }
}

fn drive_resp(body: &[u8]) -> DebugStreamServerMsg {
    DebugStreamServerMsg {
        content: Some(debug_stream_server_msg::Content::DriveResp(DebugDriveResponse {
            body: body.to_vec(),
        })),
    }
}

fn error_resp(msg: &str) -> DebugStreamServerMsg {
    DebugStreamServerMsg {
        content: Some(debug_stream_server_msg::Content::Error(DebugStreamError {
            message: msg.into(),
        })),
    }
}

/// Build a dummy stream-stop sender unused by tests that don't hit
/// the --count path. Wrapped in `Some` to match the
/// `consume_observe_response_stream` signature.
#[allow(clippy::unnecessary_wraps)]
fn dummy_stop_tx() -> Option<mpsc::Sender<reovim_protocol::v3::DebugStreamClientMsg>> {
    Some(mpsc::channel(1).0)
}

fn stream_of(
    msgs: Vec<Result<DebugStreamServerMsg, Status>>,
) -> ReceiverStream<Result<DebugStreamServerMsg, Status>> {
    let (tx, rx) = mpsc::channel(msgs.len().max(1));
    for m in msgs {
        tx.try_send(m).unwrap();
    }
    drop(tx);
    ReceiverStream::new(rx)
}

// ─────────────────────────────────────────────────────────────────────────────
// `hex_encode` + `resolve_input` + `render_*` (pure)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn hex_encode_empty() {
    assert_eq!(hex_encode(&[]), "");
}

#[test]
fn hex_encode_bytes() {
    assert_eq!(hex_encode(&[0x0a, 0xff, 0x00]), "0aff00");
}

#[test]
fn resolve_input_inline() {
    assert_eq!(resolve_input("hello").unwrap(), b"hello".to_vec());
}

#[test]
fn resolve_input_at_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.bin");
    std::fs::write(&path, b"file-bytes").unwrap();
    let spec = format!("@{}", path.display());
    assert_eq!(resolve_input(&spec).unwrap(), b"file-bytes".to_vec());
}

#[test]
fn resolve_input_at_missing_file_errors() {
    let err = resolve_input("@/nonexistent/deadbeef.bin").unwrap_err();
    assert!(err.to_string().contains("nonexistent/deadbeef.bin"));
}

#[test]
fn render_probe_plain() {
    let r = DebugProbeResponse {
        driver_name: "d".into(),
        description: "desc".into(),
        observe_schemas: vec!["o1".into(), "o2".into()],
        drive_schemas: vec!["e".into()],
    };
    let out = render_probe(&r, OutputFormat::Plain);
    assert!(out.contains("driver: d"));
    assert!(out.contains("description: desc"));
    assert!(out.contains("observe_schemas: o1, o2"));
    assert!(out.contains("drive_schemas: e"));
}

#[test]
fn render_probe_json() {
    let r = DebugProbeResponse {
        driver_name: "d".into(),
        description: "desc".into(),
        observe_schemas: vec!["o1".into()],
        drive_schemas: vec!["e".into()],
    };
    let out = render_probe(&r, OutputFormat::Json);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["driver"], "d");
    assert_eq!(v["observe_schemas"][0], "o1");
    assert_eq!(v["drive_schemas"][0], "e");
}

#[test]
fn render_frames_plain() {
    let f = vec![b"a".to_vec(), b"\x01\x02".to_vec()];
    let out = render_frames(&f, OutputFormat::Plain);
    assert!(out.contains("frame[0]=61"));
    assert!(out.contains("frame[1]=0102"));
}

#[test]
fn render_frames_json_base64() {
    let f = vec![b"hi".to_vec()];
    let out = render_frames(&f, OutputFormat::Json);
    assert!(out.contains("\"aGk=\"")); // base64("hi")
}

#[test]
fn render_drive_plain() {
    let out = render_drive(b"OK", OutputFormat::Plain);
    assert_eq!(out, "response=4f4b\n");
}

#[test]
fn render_drive_json() {
    let out = render_drive(b"OK", OutputFormat::Json);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["response"], "T0s="); // base64("OK")
}

// ─────────────────────────────────────────────────────────────────────────────
// `consume_probe_response_stream`
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn consume_probe_ok() {
    let s = stream_of(vec![Ok(probe_resp("d", &["o"], &["e"]))]);
    let out = consume_probe_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap();
    assert!(out.contains("driver: d"));
}

#[tokio::test]
async fn consume_probe_error_non_terminal_is_surfaced() {
    // Non-terminal server error before probe_resp → error on stdout,
    // verb fails.
    let s = stream_of(vec![Ok(error_resp("unknown driver: nope"))]);
    let err = consume_probe_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("unknown driver"));
}

#[tokio::test]
async fn consume_probe_stream_closed_without_resp() {
    let s = stream_of(vec![]);
    let err = consume_probe_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("stream closed"));
}

#[tokio::test]
async fn consume_probe_ignores_frames_before_probe_resp() {
    let s = stream_of(vec![
        Ok(observe_frame(b"stray")),
        Ok(probe_resp("d", &["o"], &["e"])),
    ]);
    let out = consume_probe_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap();
    assert!(out.contains("driver: d"));
}

#[tokio::test]
async fn consume_probe_propagates_tonic_status() {
    let s = stream_of(vec![Err(Status::internal("transport error"))]);
    let err = consume_probe_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("transport error"));
}

// ─────────────────────────────────────────────────────────────────────────────
// `consume_observe_response_stream`
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn consume_observe_renders_frames_to_eos() {
    let s = stream_of(vec![
        Ok(probe_resp("d", &["f"], &[])), // stray probe — should be ignored
        Ok(observe_frame(b"a")),
        Ok(observe_frame(b"b")),
        Ok(observe_frame(b"c")),
    ]);
    let out = consume_observe_response_stream(s, None, dummy_stop_tx(), OutputFormat::Plain)
        .await
        .unwrap();
    assert!(out.contains("frame[0]=61"));
    assert!(out.contains("frame[1]=62"));
    assert!(out.contains("frame[2]=63"));
}

#[tokio::test]
async fn consume_observe_count_truncates_and_sends_stop() {
    let (stop_tx, mut stop_rx) = mpsc::channel(4);
    let s = stream_of(vec![
        Ok(observe_frame(b"a")),
        Ok(observe_frame(b"b")),
        Ok(observe_frame(b"c")),
    ]);
    let out = consume_observe_response_stream(s, Some(2), Some(stop_tx), OutputFormat::Plain)
        .await
        .unwrap();
    assert!(out.contains("frame[0]=61"));
    assert!(out.contains("frame[1]=62"));
    assert!(!out.contains("frame[2]"));
    // ObserveStop must have been enqueued onto the stop channel.
    let stop_msg = stop_rx.try_recv().expect("stop msg sent");
    match stop_msg.content {
        Some(reovim_protocol::v3::debug_stream_client_msg::Content::ObserveStop(_)) => {}
        other => panic!("expected ObserveStop, got {other:?}"),
    }
}

#[tokio::test]
async fn consume_observe_error_is_surfaced() {
    let s = stream_of(vec![Ok(error_resp("bad schema"))]);
    let err = consume_observe_response_stream(s, None, dummy_stop_tx(), OutputFormat::Plain)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("bad schema"));
}

#[tokio::test]
async fn consume_observe_json_uses_base64_frames() {
    let s = stream_of(vec![Ok(observe_frame(b"hi"))]);
    let out = consume_observe_response_stream(s, None, dummy_stop_tx(), OutputFormat::Json)
        .await
        .unwrap();
    assert!(out.contains("\"aGk=\""));
}

#[tokio::test]
async fn consume_observe_propagates_tonic_status() {
    let s = stream_of(vec![Err(Status::aborted("gone"))]);
    let err = consume_observe_response_stream(s, None, dummy_stop_tx(), OutputFormat::Plain)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("gone"));
}

// ─────────────────────────────────────────────────────────────────────────────
// `consume_drive_response_stream`
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn consume_drive_ok() {
    let s = stream_of(vec![Ok(drive_resp(b"OK"))]);
    let out = consume_drive_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap();
    assert!(out.contains("response=4f4b"));
}

#[tokio::test]
async fn consume_drive_ignores_pre_resp_frames() {
    let s = stream_of(vec![Ok(probe_resp("d", &[], &["e"])), Ok(drive_resp(b"X"))]);
    let out = consume_drive_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap();
    assert!(out.contains("response=58"));
}

#[tokio::test]
async fn consume_drive_error_terminates() {
    let s = stream_of(vec![Ok(error_resp("invalid schema"))]);
    let err = consume_drive_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invalid schema"));
}

#[tokio::test]
async fn consume_drive_stream_closed_without_resp() {
    let s = stream_of(vec![]);
    let err = consume_drive_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("stream closed"));
}

#[tokio::test]
async fn consume_drive_propagates_tonic_status() {
    let s = stream_of(vec![Err(Status::unavailable("down"))]);
    let err = consume_drive_response_stream(s, OutputFormat::Plain)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("down"));
}

#[tokio::test]
async fn consume_drive_json() {
    let s = stream_of(vec![Ok(drive_resp(b"OK"))]);
    let out = consume_drive_response_stream(s, OutputFormat::Json)
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["response"], "T0s=");
}

// ─────────────────────────────────────────────────────────────────────────────
// Outbound stream builders — pure property tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn build_probe_client_stream_yields_select() {
    use {super::build_probe_client_stream, tokio_stream::StreamExt};
    let stream = build_probe_client_stream("xyz");
    tokio::pin!(stream);
    let first = stream.next().await.expect("one message");
    match first.content {
        Some(reovim_protocol::v3::debug_stream_client_msg::Content::Select(s)) => {
            assert_eq!(s.driver_name, "xyz");
        }
        other => panic!("expected Select, got {other:?}"),
    }
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn build_observe_client_stream_yields_select_then_observe_start() {
    use {super::build_observe_client_stream, tokio_stream::StreamExt};
    let (stream, _stop_tx) = build_observe_client_stream("drv", "sch");
    tokio::pin!(stream);
    let first = stream.next().await.unwrap();
    assert!(matches!(
        first.content,
        Some(reovim_protocol::v3::debug_stream_client_msg::Content::Select(_))
    ));
    let second = stream.next().await.unwrap();
    match second.content {
        Some(reovim_protocol::v3::debug_stream_client_msg::Content::ObserveStart(o)) => {
            assert_eq!(o.schema, "sch");
        }
        other => panic!("expected ObserveStart, got {other:?}"),
    }
}

#[tokio::test]
async fn build_drive_client_stream_yields_select_then_drive_cmd() {
    use {super::build_drive_client_stream, tokio_stream::StreamExt};
    let stream = build_drive_client_stream("drv", "sch", b"body".to_vec());
    tokio::pin!(stream);
    let first = stream.next().await.unwrap();
    assert!(matches!(
        first.content,
        Some(reovim_protocol::v3::debug_stream_client_msg::Content::Select(_))
    ));
    let second = stream.next().await.unwrap();
    match second.content {
        Some(reovim_protocol::v3::debug_stream_client_msg::Content::DriveCmd(d)) => {
            assert_eq!(d.schema, "sch");
            assert_eq!(d.body, b"body".to_vec());
        }
        other => panic!("expected DriveCmd, got {other:?}"),
    }
}
