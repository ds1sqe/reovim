//! Server mode entry point
//!
//! Starts reovim in JSON-RPC server mode, accepting commands via multiple transports:
//! - Stdio: For process piping (spawned by parent)
//! - Unix socket: For local IPC
//! - TCP: For network access

use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use {
    reovim_core::{
        event::InnerEvent,
        io::{input::ChannelKeySource, output::DualOutput},
        rpc::{
            RpcNotification, RpcRequest, RpcResponse, TransportConfig, TransportConnection,
            TransportListener,
            server::{RpcServer, run_reader, run_server_loop, run_writer},
        },
        runtime::Runtime,
        screen::Screen,
    },
    tokio::sync::mpsc,
};

/// Shared state for connection management in persistent server mode
struct ConnectionState {
    /// Sender for responses to the current active connection
    response_tx: Mutex<Option<mpsc::Sender<RpcResponse>>>,
    /// Sender for notifications to the current active connection
    notification_tx: Mutex<Option<mpsc::Sender<RpcNotification>>>,
    /// Number of active connections
    connection_count: AtomicUsize,
    /// Channel to signal shutdown (test mode only)
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Whether test mode is enabled
    test_mode: bool,
}

/// Run the editor in server mode with the specified transport
#[allow(clippy::future_not_send)]
#[allow(clippy::too_many_lines)]
pub async fn run_server(
    file_path: Option<String>,
    dual_output: bool,
    transport_config: TransportConfig,
    test_mode: bool,
) -> Result<(), io::Error> {
    // Default screen size for headless mode
    let width = 80;
    let height = 24;

    // Create screen with optional terminal output
    let (screen, capture_handle) = if dual_output {
        // Dual mode: render to terminal AND capture
        reovim_core::command::terminal::enable_raw_mode()?;

        let dual = DualOutput::new(Box::new(io::stdout()));
        let handle = dual.capture_handle();

        let mut screen = Screen::with_writer(dual, width, height);
        screen.initialize()?;

        (screen, Some(handle))
    } else {
        // Headless mode: capture only, no terminal
        let dual = DualOutput::headless();
        let handle = dual.capture_handle();

        let screen = Screen::with_writer(dual, width, height);
        // Don't call initialize() in headless mode - no terminal to initialize

        (screen, Some(handle))
    };

    // Create channels for key injection
    let (key_tx, key_source) = ChannelKeySource::new();

    // Create runtime with the screen
    // Enable frame buffer rendering to avoid multiple screen clears (reduces flickering)
    let runtime = Runtime::new(screen)
        .with_file(file_path)
        .with_render_strategy(reovim_core::frame::RenderStrategyConfig::VirtualBuffer);

    // Get the event sender from runtime (clone for later use)
    let event_tx = runtime.tx.clone();
    let event_tx_for_shutdown = event_tx.clone();

    // In dual mode, spawn a task to read terminal events and forward to key channel
    if dual_output {
        let key_tx_terminal = key_tx.clone();
        let event_tx_resize = event_tx.clone();
        tokio::spawn(async move {
            use {
                futures::StreamExt,
                reovim_sys::event::{Event, EventStream, KeyEventKind},
            };

            let mut stream = EventStream::new();
            while let Some(result) = stream.next().await {
                match result {
                    Ok(Event::Key(key_event)) => {
                        if key_event.kind == KeyEventKind::Press
                            && key_tx_terminal.send(key_event).await.is_err()
                        {
                            break; // Channel closed
                        }
                    }
                    Ok(Event::Resize(cols, rows)) => {
                        let _ = event_tx_resize
                            .send(InnerEvent::ScreenResizeEvent {
                                width: cols,
                                height: rows,
                            })
                            .await;
                    }
                    Ok(_) => {} // Ignore other events (Mouse, Focus, Paste)
                    Err(_) => break,
                }
            }
        });
    }

    // Create channels for RPC communication
    let (request_tx, request_rx) = mpsc::channel::<RpcRequest>(256);
    let (response_tx, response_rx) = mpsc::channel::<RpcResponse>(256);
    let (notification_tx, notification_rx) = mpsc::channel::<RpcNotification>(256);

    // Create RPC server
    let server = Arc::new(RpcServer::new(event_tx, key_tx, capture_handle, notification_tx));

    // Set up transport based on configuration
    match &transport_config {
        TransportConfig::Stdio => {
            // Stdio mode: use stdin/stdout directly (always one-shot)
            let conn = TransportConnection::stdio();
            tokio::spawn(run_reader(conn.reader, request_tx));
            tokio::spawn(run_writer(conn.writer, response_rx, notification_rx));

            // Spawn RPC server loop for stdio
            let server_clone = Arc::clone(&server);
            tokio::spawn(run_server_loop(server_clone, request_rx, response_tx));
        }
        TransportConfig::UnixSocket { .. } | TransportConfig::Tcp { .. } => {
            // Listener mode: persistent server
            let listener = TransportListener::bind(&transport_config)
                .await?
                .expect("listener should be Some for non-stdio transport");

            // Create shutdown channel for test mode
            let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

            let conn_state = Arc::new(ConnectionState {
                response_tx: Mutex::new(None),
                notification_tx: Mutex::new(None),
                connection_count: AtomicUsize::new(0),
                shutdown_tx: if test_mode { Some(shutdown_tx) } else { None },
                test_mode,
            });

            // Spawn persistent server loop (uses shared state for responses)
            let server_clone = Arc::clone(&server);
            let conn_state_clone = Arc::clone(&conn_state);
            tokio::spawn(run_server_loop_persistent(server_clone, request_rx, conn_state_clone));

            // Accept loop (runs in background)
            let conn_state_accept = Arc::clone(&conn_state);
            tokio::spawn(async move {
                loop {
                    tracing::info!("Waiting for client connection...");
                    match listener.accept().await {
                        Ok(conn) => {
                            conn_state_accept
                                .connection_count
                                .fetch_add(1, Ordering::SeqCst);
                            let count = conn_state_accept.connection_count.load(Ordering::SeqCst);
                            tracing::info!("Client connected (active connections: {count})");

                            // Fresh channels for this connection
                            let (resp_tx, resp_rx) = mpsc::channel::<RpcResponse>(256);
                            let (notif_tx, notif_rx) = mpsc::channel::<RpcNotification>(256);

                            // Update shared state with new connection's channels
                            *conn_state_accept.response_tx.lock().unwrap() = Some(resp_tx);
                            *conn_state_accept.notification_tx.lock().unwrap() = Some(notif_tx);

                            // Spawn connection handler that tracks disconnect
                            let state = Arc::clone(&conn_state_accept);
                            tokio::spawn(handle_connection(
                                conn,
                                request_tx.clone(),
                                resp_rx,
                                notif_rx,
                                state,
                            ));
                        }
                        Err(e) => {
                            tracing::error!("Accept error: {e}");
                        }
                    }
                }
            });

            // In test mode: wait for shutdown signal, then trigger runtime exit
            if test_mode {
                tokio::spawn(async move {
                    shutdown_rx.recv().await;
                    tracing::info!("Test mode: all clients disconnected, shutting down");
                    let _ = event_tx_for_shutdown.send(InnerEvent::KillSignal).await;
                });
            }
        }
    }

    // Run the runtime event loop with key injection source
    // Note: We need to use the server-mode init that uses ChannelKeySource
    runtime.init_with_key_source(key_source).await;

    // Cleanup
    if dual_output {
        reovim_core::command::terminal::disable_raw_mode()?;
    }

    tracing::info!("Server mode shutting down");
    Ok(())
}

/// Handle a single client connection, tracking its lifecycle
async fn handle_connection(
    conn: TransportConnection,
    request_tx: mpsc::Sender<RpcRequest>,
    response_rx: mpsc::Receiver<RpcResponse>,
    notification_rx: mpsc::Receiver<RpcNotification>,
    state: Arc<ConnectionState>,
) {
    // Spawn reader and writer for this connection
    let reader_handle = tokio::spawn(run_reader(conn.reader, request_tx));
    let writer_handle = tokio::spawn(run_writer(conn.writer, response_rx, notification_rx));

    // Wait for either to finish (connection closed)
    tokio::select! {
        _ = reader_handle => {}
        _ = writer_handle => {}
    }

    // Decrement connection count
    let prev = state.connection_count.fetch_sub(1, Ordering::SeqCst);
    let new_count = prev - 1;
    tracing::info!("Client disconnected (active connections: {new_count})");

    // In test mode: if all clients gone, trigger shutdown
    if state.test_mode
        && prev == 1
        && let Some(ref tx) = state.shutdown_tx
    {
        let _ = tx.send(()).await;
    }
}

/// Server loop that uses shared state to route responses to the current connection
async fn run_server_loop_persistent(
    server: Arc<RpcServer>,
    mut request_rx: mpsc::Receiver<RpcRequest>,
    conn_state: Arc<ConnectionState>,
) {
    while let Some(request) = request_rx.recv().await {
        // handle_request returns None for notifications (no response needed)
        if let Some(response) = server.handle_request(request).await {
            // Send to current connection (if any)
            let tx = conn_state.response_tx.lock().unwrap().clone();
            if let Some(tx) = tx {
                // Ignore send errors (client may have disconnected)
                let _ = tx.send(response).await;
            }
        }
    }
}
