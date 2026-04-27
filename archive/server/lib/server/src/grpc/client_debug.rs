//! `ClientDebugService` gRPC implementation (#770 Phase 1).
//!
//! Bidirectional streaming handler that routes inspector operations
//! (probe / observe / drive) against drivers held by a
//! [`ClientDebugRegistry`].
//!
//! # State machine
//!
//! Each `DebugStream` has two states:
//!
//! - **`NotSelected`**: only `DebugSelect` is accepted. Any other
//!   client message yields a non-terminal `DebugStreamError`.
//! - **`Selected(driver_name)`**: `DebugObserveStart`, `DebugObserveStop`,
//!   `DebugDriveCommand`, and further `DebugSelect` messages are
//!   accepted. A second `Select` cancels any active observer and
//!   switches driver.
//!
//! # Observer pump
//!
//! `DebugObserveStart` spawns an async pump task that owns the
//! registry's `ObserverPump`. Frames stream through a shared
//! `tokio::sync::mpsc` channel back to the client. The pump's
//! cancellation oneshot fires when:
//!
//! 1. The client sends `DebugObserveStop`.
//! 2. The client sends a second `DebugSelect`.
//! 3. The client drops the stream (outer task ends → cleanup block).
//!
//! # Error semantics
//!
//! - Driver errors (`RegistryError::DriverError`) surface as
//!   `DebugStreamError`; the stream stays open.
//! - Loader errors (`RegistryError::Loader`, `UnknownDriver`)
//!   surface as `DebugStreamError`; the stream stays open.
//! - Driver panics (`RegistryError::DriverPanicked`) close the
//!   stream with `Status::internal` and evict the driver from the
//!   registry.

#![allow(clippy::result_large_err)]

use {
    crate::client_debug_registry::{ClientDebugRegistry, RegistryError},
    reovim_client_subsys_debug::DebugProbe,
    reovim_protocol::v3::{
        DebugDriveResponse, DebugObserveFrame, DebugProbeResponse, DebugStreamClientMsg,
        DebugStreamError, DebugStreamServerMsg, client_debug_service_server::ClientDebugService,
        debug_stream_client_msg, debug_stream_server_msg,
    },
    std::{pin::Pin, sync::Arc},
    tokio::{
        sync::{mpsc, oneshot},
        task::JoinHandle,
    },
    tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream},
    tonic::{Request, Response, Status, Streaming},
};

/// Outbound channel buffer. Deep enough to absorb brief scheduler
/// hiccups without back-pressuring the driver pump.
const OUTBOUND_BUFFER: usize = 32;

/// `ClientDebugService` gRPC implementation.
pub struct ClientDebugServiceImpl {
    registry: Arc<ClientDebugRegistry>,
}

impl ClientDebugServiceImpl {
    /// Create a handler that routes against the given registry.
    #[must_use]
    pub const fn new(registry: Arc<ClientDebugRegistry>) -> Self {
        Self { registry }
    }
}

type OutTx = mpsc::Sender<Result<DebugStreamServerMsg, Status>>;

struct ObserverSlot {
    cancel: oneshot::Sender<()>,
    handle: JoinHandle<()>,
}

#[tonic::async_trait]
impl ClientDebugService for ClientDebugServiceImpl {
    type DebugStreamStream =
        Pin<Box<dyn Stream<Item = Result<DebugStreamServerMsg, Status>> + Send + 'static>>;

    async fn debug_stream(
        &self,
        request: Request<Streaming<DebugStreamClientMsg>>,
    ) -> Result<Response<Self::DebugStreamStream>, Status> {
        let client_stream = request.into_inner();
        let (tx, rx) = mpsc::channel::<Result<DebugStreamServerMsg, Status>>(OUTBOUND_BUFFER);
        let registry = Arc::clone(&self.registry);

        tokio::spawn(async move {
            run_stream(registry, client_stream, tx).await;
        });

        let out_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(out_stream) as Self::DebugStreamStream))
    }
}

async fn run_stream<S>(registry: Arc<ClientDebugRegistry>, mut client_stream: S, tx: OutTx)
where
    S: Stream<Item = Result<DebugStreamClientMsg, Status>> + Unpin + Send,
{
    let mut selected: Option<String> = None;
    let mut observer: Option<ObserverSlot> = None;

    // Client half-close (no more client messages) does NOT end the
    // stream; the server may still be pumping observer frames to the
    // inspector. We only break out of the loop if there is no active
    // observer when the client half-closes (there is nothing left to
    // do), OR on transport error.
    while let Some(msg_result) = client_stream.next().await {
        let Ok(msg) = msg_result else {
            break; // transport error → exit & cancel observer
        };

        let Some(content) = msg.content else {
            send_error(&tx, "empty client message").await;
            continue;
        };

        match content {
            debug_stream_client_msg::Content::Select(sel) => {
                cancel_observer(observer.take()).await;
                // `probe_async` is infallible under `DebugDriverHandle`
                // (probe returns `DebugProbe` by value); the only
                // error path is `UnknownDriver`. Driver-level panics
                // cannot surface here because the vtable's probe slot
                // is itself infallible (Phase 0 design).
                match registry.probe_async(&sel.driver_name).await {
                    Ok(probe) => {
                        selected = Some(sel.driver_name.clone());
                        send_probe(&tx, &sel.driver_name, &probe).await;
                    }
                    Err(e) => {
                        send_error(&tx, &e.to_string()).await;
                    }
                }
            }
            debug_stream_client_msg::Content::ObserveStart(start) => {
                let Some(name) = selected.clone() else {
                    send_error(&tx, "driver not selected").await;
                    continue;
                };
                cancel_observer(observer.take()).await;
                let (cancel_tx, cancel_rx) = oneshot::channel();
                let handle = tokio::spawn(observer_pump(
                    Arc::clone(&registry),
                    name,
                    start.schema,
                    tx.clone(),
                    cancel_rx,
                ));
                observer = Some(ObserverSlot {
                    cancel: cancel_tx,
                    handle,
                });
            }
            debug_stream_client_msg::Content::ObserveStop(_) => {
                cancel_observer(observer.take()).await;
            }
            debug_stream_client_msg::Content::DriveCmd(drive) => {
                let Some(name) = selected.clone() else {
                    send_error(&tx, "driver not selected").await;
                    continue;
                };
                match registry.drive(&name, &drive.body).await {
                    Ok(body) => {
                        let _ = tx
                            .send(Ok(DebugStreamServerMsg {
                                content: Some(debug_stream_server_msg::Content::DriveResp(
                                    DebugDriveResponse { body },
                                )),
                            }))
                            .await;
                    }
                    Err(RegistryError::DriverPanicked) => {
                        registry.evict_on_panic(&name);
                        let _ = tx.send(Err(Status::internal("driver panicked"))).await;
                        return;
                    }
                    Err(e) => {
                        send_error(&tx, &e.to_string()).await;
                    }
                }
            }
        }
    }

    // Main loop ended. If an observer is running, wait for its natural
    // completion (EOS or outbound send failure). Dropping the outer
    // `tx` (our clone) does NOT close the outbound channel: the pump
    // still holds its clone of `tx`. The pump terminates when it
    // emits EOS, when the driver panics, or when the outbound send
    // fails (client fully disconnected, tonic drops the receiver).
    drop(tx); // release our clone so the pump's clone is the only owner
    if let Some(slot) = observer.take() {
        let _ = slot.handle.await;
        drop(slot.cancel);
    }
}

async fn observer_pump(
    registry: Arc<ClientDebugRegistry>,
    driver_name: String,
    schema: String,
    tx: OutTx,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    let mut pump = match registry.observe(&driver_name, schema.as_bytes()).await {
        Ok(p) => p,
        Err(RegistryError::DriverPanicked) => {
            registry.evict_on_panic(&driver_name);
            let _ = tx.send(Err(Status::internal("driver panicked"))).await;
            return;
        }
        Err(e) => {
            send_error(&tx, &e.to_string()).await;
            return;
        }
    };

    loop {
        let frame_result = pump.next_frame();
        match frame_result {
            Ok(None) => break, // EOS — observer complete
            Ok(Some(body)) => {
                let msg = DebugStreamServerMsg {
                    content: Some(debug_stream_server_msg::Content::ObserveFrame(
                        DebugObserveFrame { body },
                    )),
                };
                tokio::select! {
                    biased;
                    _ = &mut cancel_rx => return,
                    send_result = tx.send(Ok(msg)) => {
                        if send_result.is_err() {
                            return; // outbound channel closed
                        }
                    }
                }
            }
            Err(RegistryError::DriverPanicked) => {
                // Dropping pump releases the owned mutex guard before
                // evict_on_panic takes the outer write lock.
                drop(pump);
                registry.evict_on_panic(&driver_name);
                let _ = tx.send(Err(Status::internal("driver panicked"))).await;
                return;
            }
            Err(e) => {
                let msg = e.to_string();
                tokio::select! {
                    biased;
                    _ = &mut cancel_rx => return,
                    () = send_error(&tx, &msg) => {}
                }
                // Non-terminal: continue pumping. The driver may
                // recover on its next frame or EOS cleanly.
            }
        }
    }
}

async fn cancel_observer(slot: Option<ObserverSlot>) {
    if let Some(slot) = slot {
        let _ = slot.cancel.send(());
        // Await task completion so the ObserverPump (and its lock
        // guard) is released before we move on. A panicked task is
        // acceptable; we only need shape.
        let _ = slot.handle.await;
    }
}

async fn send_error(tx: &OutTx, message: &str) {
    let _ = tx
        .send(Ok(DebugStreamServerMsg {
            content: Some(debug_stream_server_msg::Content::Error(DebugStreamError {
                message: message.to_owned(),
            })),
        }))
        .await;
}

async fn send_probe(tx: &OutTx, driver_name: &str, probe: &DebugProbe) {
    let _ = tx
        .send(Ok(DebugStreamServerMsg {
            content: Some(debug_stream_server_msg::Content::ProbeResp(probe_to_response(
                driver_name,
                probe,
            ))),
        }))
        .await;
}

fn probe_to_response(driver_name: &str, probe: &DebugProbe) -> DebugProbeResponse {
    DebugProbeResponse {
        driver_name: driver_name.to_owned(),
        description: cstr_to_string(&probe.description),
        observe_schemas: schema_list(probe.observe_schemas_count, &probe.observe_schemas),
        drive_schemas: schema_list(probe.drive_schemas_count, &probe.drive_schemas),
    }
}

fn cstr_to_string(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

fn schema_list<const N: usize>(count: u32, slots: &[[u8; N]]) -> Vec<String> {
    let n = (count as usize).min(slots.len());
    slots[..n].iter().map(|s| cstr_to_string(s)).collect()
}

#[cfg(test)]
#[path = "client_debug_tests.rs"]
mod tests;
