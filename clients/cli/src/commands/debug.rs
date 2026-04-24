//! Driver-owned debug-surface CLI verbs (#770 Phase 2).
//!
//! Three verbs — `probe`, `observe`, `drive` — open a single
//! `ClientDebugService::DebugStream` each and route opaque bytes to
//! the driver. The CLI never decodes schema-specific payloads; only
//! driver + schema names are structured at the flag layer.
//!
//! # Testability split
//!
//! Each verb is factored into two halves:
//! - `build_*_client_stream(...)` — pure function producing the
//!   outbound `DebugStreamClientMsg` sequence for this verb's args.
//! - `consume_*_response_stream(stream, format)` — consumes an
//!   `impl Stream<Item = Result<DebugStreamServerMsg, Status>>` and
//!   returns the rendered output string.
//!
//! Unit tests feed synthetic `Result<DebugStreamServerMsg, Status>`
//! streams into the consume half without standing up a tonic server.
//! The top-level `probe` / `observe` / `drive` functions compose the
//! two halves against the real `GrpcClient::debug_stream`.

use {
    crate::{DebugSubcommand, GrpcClient, GrpcClientError, OutputFormat},
    base64::{Engine as _, engine::general_purpose::STANDARD as BASE64},
    reovim_protocol::v3::{
        DebugDriveCommand, DebugObserveStart, DebugObserveStop, DebugSelect,
        DebugStreamClientMsg, DebugStreamServerMsg, debug_stream_client_msg,
        debug_stream_server_msg,
    },
    std::{fmt::Write, fs, path::Path},
    tokio::sync::mpsc,
    tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream},
    tonic::Status,
};

/// Dispatch a `debug` subcommand to the matching verb.
///
/// # Errors
/// Propagates gRPC / I/O errors from the underlying verbs.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn dispatch(
    client: &mut GrpcClient,
    subcommand: &DebugSubcommand,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    match subcommand {
        DebugSubcommand::Probe { driver } => probe(client, driver, format).await,
        DebugSubcommand::Observe {
            driver,
            schema,
            count,
        } => observe(client, driver, schema, *count, format).await,
        DebugSubcommand::Drive {
            driver,
            schema,
            input,
        } => {
            let bytes = resolve_input(input)?;
            drive(client, driver, schema, bytes, format).await
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// `probe` verb
// ─────────────────────────────────────────────────────────────────────────────

/// Open a stream, send `Select`, consume until `ProbeResp` arrives.
///
/// # Errors
/// Propagates `GrpcClientError` from the transport or the server's
/// non-terminal `DebugStreamError`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn probe(
    client: &mut GrpcClient,
    driver: &str,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let outbound = build_probe_client_stream(driver);
    let stream = client.debug_stream(outbound).await?;
    consume_probe_response_stream(stream, format).await
}

fn build_probe_client_stream(driver: &str) -> impl Stream<Item = DebugStreamClientMsg> + Send + 'static {
    let (tx, rx) = mpsc::channel(2);
    let driver = driver.to_owned();
    tokio::spawn(async move {
        let _ = tx
            .send(DebugStreamClientMsg {
                content: Some(debug_stream_client_msg::Content::Select(DebugSelect {
                    driver_name: driver,
                })),
            })
            .await;
    });
    ReceiverStream::new(rx)
}

async fn consume_probe_response_stream(
    mut stream: impl Stream<Item = Result<DebugStreamServerMsg, Status>> + Unpin,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    while let Some(msg) = stream.next().await {
        let msg = msg?;
        match msg.content {
            Some(debug_stream_server_msg::Content::ProbeResp(r)) => {
                return Ok(render_probe(&r, format));
            }
            Some(debug_stream_server_msg::Content::Error(e)) => {
                return Err(GrpcClientError::OperationFailed(e.message));
            }
            _ => {} // ignore stray frames before probe_resp
        }
    }
    Err(GrpcClientError::OperationFailed(
        "stream closed before probe response".into(),
    ))
}

fn render_probe(
    r: &reovim_protocol::v3::DebugProbeResponse,
    format: OutputFormat,
) -> String {
    match format {
        OutputFormat::Plain => {
            let mut out = String::new();
            let _ = writeln!(out, "driver: {}", r.driver_name);
            let _ = writeln!(out, "description: {}", r.description);
            let _ = writeln!(out, "observe_schemas: {}", r.observe_schemas.join(", "));
            let _ = writeln!(out, "drive_schemas: {}", r.drive_schemas.join(", "));
            out
        }
        OutputFormat::Json => serde_json::json!({
            "driver": r.driver_name,
            "description": r.description,
            "observe_schemas": r.observe_schemas,
            "drive_schemas": r.drive_schemas,
        })
        .to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// `observe` verb
// ─────────────────────────────────────────────────────────────────────────────

/// Open a stream, send `Select` + `ObserveStart`, render frames until
/// EOS or `--count N` is reached.
///
/// # Errors
/// Propagates `GrpcClientError` from the transport or the server's
/// non-terminal `DebugStreamError`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn observe(
    client: &mut GrpcClient,
    driver: &str,
    schema: &str,
    count: Option<u32>,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let (outbound, stop_tx) = build_observe_client_stream(driver, schema);
    let stream = client.debug_stream(outbound).await?;
    // Without `--count` we stay passive and let the server close the
    // outbound when the pump naturally EOSes. Drop the remaining
    // client-side sender so the HTTP/2 stream half-closes; Phase 1's
    // handler waits for the observer pump to finish after client EOF.
    let stop_tx_opt = if count.is_some() {
        Some(stop_tx)
    } else {
        drop(stop_tx);
        None
    };
    consume_observe_response_stream(stream, count, stop_tx_opt, format).await
}

fn build_observe_client_stream(
    driver: &str,
    schema: &str,
) -> (
    impl Stream<Item = DebugStreamClientMsg> + Send + 'static,
    mpsc::Sender<DebugStreamClientMsg>,
) {
    let (tx, rx) = mpsc::channel::<DebugStreamClientMsg>(4);
    let driver = driver.to_owned();
    let schema = schema.to_owned();
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let _ = tx_clone
            .send(DebugStreamClientMsg {
                content: Some(debug_stream_client_msg::Content::Select(DebugSelect {
                    driver_name: driver,
                })),
            })
            .await;
        let _ = tx_clone
            .send(DebugStreamClientMsg {
                content: Some(debug_stream_client_msg::Content::ObserveStart(
                    DebugObserveStart { schema },
                )),
            })
            .await;
    });
    (ReceiverStream::new(rx), tx)
}

async fn consume_observe_response_stream(
    mut stream: impl Stream<Item = Result<DebugStreamServerMsg, Status>> + Unpin,
    count: Option<u32>,
    stop_tx: Option<mpsc::Sender<DebugStreamClientMsg>>,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let mut frames: Vec<Vec<u8>> = Vec::new();
    while let Some(msg) = stream.next().await {
        let msg = msg?;
        match msg.content {
            Some(debug_stream_server_msg::Content::ObserveFrame(f)) => {
                frames.push(f.body);
                if let Some(limit) = count
                    && u32::try_from(frames.len()).unwrap_or(u32::MAX) >= limit
                {
                    if let Some(tx) = stop_tx {
                        let _ = tx
                            .send(DebugStreamClientMsg {
                                content: Some(debug_stream_client_msg::Content::ObserveStop(
                                    DebugObserveStop {},
                                )),
                            })
                            .await;
                    }
                    break;
                }
            }
            Some(debug_stream_server_msg::Content::Error(e)) => {
                return Err(GrpcClientError::OperationFailed(e.message));
            }
            _ => {}
        }
    }
    Ok(render_frames(&frames, format))
}

fn render_frames(frames: &[Vec<u8>], format: OutputFormat) -> String {
    match format {
        OutputFormat::Plain => {
            let mut out = String::new();
            for (i, f) in frames.iter().enumerate() {
                let _ = writeln!(out, "frame[{i}]={}", hex_encode(f));
            }
            out
        }
        OutputFormat::Json => {
            let encoded: Vec<String> = frames.iter().map(|f| BASE64.encode(f)).collect();
            serde_json::json!({ "frames": encoded }).to_string()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// `drive` verb
// ─────────────────────────────────────────────────────────────────────────────

/// Open a stream, send `Select` + `DriveCommand`, render the response.
///
/// # Errors
/// Propagates `GrpcClientError` from the transport or the server's
/// non-terminal `DebugStreamError`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn drive(
    client: &mut GrpcClient,
    driver: &str,
    schema: &str,
    body: Vec<u8>,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    let outbound = build_drive_client_stream(driver, schema, body);
    let stream = client.debug_stream(outbound).await?;
    consume_drive_response_stream(stream, format).await
}

fn build_drive_client_stream(
    driver: &str,
    schema: &str,
    body: Vec<u8>,
) -> impl Stream<Item = DebugStreamClientMsg> + Send + 'static {
    let (tx, rx) = mpsc::channel(2);
    let driver = driver.to_owned();
    let schema = schema.to_owned();
    tokio::spawn(async move {
        let _ = tx
            .send(DebugStreamClientMsg {
                content: Some(debug_stream_client_msg::Content::Select(DebugSelect {
                    driver_name: driver,
                })),
            })
            .await;
        let _ = tx
            .send(DebugStreamClientMsg {
                content: Some(debug_stream_client_msg::Content::DriveCmd(
                    DebugDriveCommand { schema, body },
                )),
            })
            .await;
    });
    ReceiverStream::new(rx)
}

async fn consume_drive_response_stream(
    mut stream: impl Stream<Item = Result<DebugStreamServerMsg, Status>> + Unpin,
    format: OutputFormat,
) -> Result<String, GrpcClientError> {
    while let Some(msg) = stream.next().await {
        let msg = msg?;
        match msg.content {
            Some(debug_stream_server_msg::Content::DriveResp(r)) => {
                return Ok(render_drive(&r.body, format));
            }
            Some(debug_stream_server_msg::Content::Error(e)) => {
                return Err(GrpcClientError::OperationFailed(e.message));
            }
            _ => {}
        }
    }
    Err(GrpcClientError::OperationFailed(
        "stream closed before drive response".into(),
    ))
}

fn render_drive(body: &[u8], format: OutputFormat) -> String {
    match format {
        OutputFormat::Plain => format!("response={}\n", hex_encode(body)),
        OutputFormat::Json => serde_json::json!({ "response": BASE64.encode(body) }).to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve a `--input SPEC` value: `@path` reads a file, otherwise
/// returns the spec bytes as UTF-8.
#[allow(clippy::result_large_err)]
fn resolve_input(spec: &str) -> Result<Vec<u8>, GrpcClientError> {
    spec.strip_prefix('@').map_or_else(
        || Ok(spec.as_bytes().to_vec()),
        |path| {
            fs::read(Path::new(path)).map_err(|e| {
                GrpcClientError::InvalidArgument(format!("failed to read {path}: {e}"))
            })
        },
    )
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[cfg(test)]
#[path = "debug_tests.rs"]
mod tests;
