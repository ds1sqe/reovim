//! Port Manager daemon implementation.
//!
//! The daemon listens on a TCP port (default 12521) and provides
//! a central registry for instance discovery.

use std::{
    io::{self, Write},
    sync::Arc,
    time::Duration,
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::watch,
};

use crate::server::instance::InstanceRegistry;

use super::{
    MANAGER_HOST, MANAGER_PORT,
    protocol::{ManagerMethod, ManagerRequest, ManagerResponse},
};

/// Health check interval in seconds.
const HEALTH_CHECK_INTERVAL_SECS: u64 = 30;

/// Manager daemon.
///
/// Provides central registry and health checking for instance discovery.
pub struct ManagerDaemon {
    listener: TcpListener,
    registry: Arc<InstanceRegistry>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl ManagerDaemon {
    /// Bind the manager daemon to the default address.
    ///
    /// # Errors
    ///
    /// Returns an error if binding fails (e.g., port in use).
    pub async fn bind() -> io::Result<Self> {
        Self::bind_addr(MANAGER_HOST, MANAGER_PORT).await
    }

    /// Bind the manager daemon to a specific address.
    ///
    /// # Errors
    ///
    /// Returns an error if binding fails.
    pub async fn bind_addr(host: &str, port: u16) -> io::Result<Self> {
        let listener = TcpListener::bind((host, port)).await.map_err(|e| {
            if e.kind() == io::ErrorKind::AddrInUse {
                io::Error::new(
                    e.kind(),
                    format!(
                        "Port {port} in use. Manager may already be running. \
                         Check with 'reovim manager status'"
                    ),
                )
            } else {
                e
            }
        })?;

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Ok(Self {
            listener,
            registry: Arc::new(InstanceRegistry::new()),
            shutdown_tx,
            shutdown_rx,
        })
    }

    /// Get the local address the daemon is bound to.
    ///
    /// # Errors
    ///
    /// Returns an error if the address cannot be determined.
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Run the manager daemon.
    ///
    /// # Arguments
    ///
    /// * `ready_signal` - If true, print `READY <addr>` to stdout when bound.
    ///
    /// # Errors
    ///
    /// Returns an error if the accept loop fails.
    pub async fn run(&self, ready_signal: bool) -> io::Result<()> {
        let addr = self.local_addr()?;

        // Signal ready if requested
        if ready_signal {
            println!("READY {addr}");
            std::io::stdout().flush().ok();
        } else {
            eprintln!("Manager listening on {addr}");
        }

        // Start health check task
        let registry = Arc::clone(&self.registry);
        let mut health_shutdown_rx = self.shutdown_rx.clone();
        tokio::spawn(async move {
            Self::health_check_loop(registry, &mut health_shutdown_rx).await;
        });

        // Accept loop
        let mut shutdown_rx = self.shutdown_rx.clone();
        loop {
            tokio::select! {
                result = self.listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            tracing::debug!("Manager connection from {addr}");
                            let registry = Arc::clone(&self.registry);
                            let shutdown_tx = self.shutdown_tx.clone();
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_client(stream, registry, shutdown_tx).await {
                                    tracing::warn!("Manager client error: {e}");
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Manager accept error: {e}");
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Manager shutting down");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle a single client connection.
    async fn handle_client(
        stream: TcpStream,
        registry: Arc<InstanceRegistry>,
        shutdown_tx: watch::Sender<bool>,
    ) -> io::Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                break; // EOF
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Parse request
            let request: ManagerRequest = match serde_json::from_str(line) {
                Ok(req) => req,
                Err(e) => {
                    let response = ManagerResponse::error(0, -32700, format!("Parse error: {e}"));
                    let response_json = serde_json::to_string(&response)?;
                    writer.write_all(response_json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    continue;
                }
            };

            // Handle request
            let response = Self::handle_request(&request, &registry, &shutdown_tx);

            // Send response
            let response_json = serde_json::to_string(&response)?;
            writer.write_all(response_json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;

            // Check if shutdown was requested
            if matches!(request.method, ManagerMethod::Shutdown) {
                break;
            }
        }

        Ok(())
    }

    /// Handle a single request.
    fn handle_request(
        request: &ManagerRequest,
        registry: &InstanceRegistry,
        shutdown_tx: &watch::Sender<bool>,
    ) -> ManagerResponse {
        match &request.method {
            ManagerMethod::Ping => {
                ManagerResponse::ok(request.id, serde_json::json!({"status": "ok"}))
            }

            ManagerMethod::List => match registry.list() {
                Ok(instances) => {
                    let json = serde_json::to_value(&instances).unwrap_or_default();
                    ManagerResponse::ok(request.id, json)
                }
                Err(e) => ManagerResponse::error(request.id, -32000, e.to_string()),
            },

            ManagerMethod::Query { name } => match registry.get(name) {
                Ok(Some(info)) => {
                    let json = serde_json::to_value(&info).unwrap_or_default();
                    ManagerResponse::ok(request.id, json)
                }
                Ok(None) => ManagerResponse::not_found(request.id, name),
                Err(e) => ManagerResponse::error(request.id, -32000, e.to_string()),
            },

            ManagerMethod::Register(info) => match registry.register(info) {
                Ok(()) => ManagerResponse::ok(request.id, serde_json::json!({"status": "ok"})),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    ManagerResponse::already_exists(request.id, &info.name)
                }
                Err(e) => ManagerResponse::error(request.id, -32000, e.to_string()),
            },

            ManagerMethod::Unregister { name } => match registry.unregister(name) {
                Ok(()) => ManagerResponse::ok(request.id, serde_json::json!({"status": "ok"})),
                Err(e) => ManagerResponse::error(request.id, -32000, e.to_string()),
            },

            ManagerMethod::Shutdown => {
                // Signal shutdown
                let _ = shutdown_tx.send(true);
                ManagerResponse::ok(request.id, serde_json::json!({"status": "shutting_down"}))
            }
        }
    }

    /// Periodic health check loop.
    ///
    /// Runs `registry.list()` periodically to clean up stale entries.
    async fn health_check_loop(
        registry: Arc<InstanceRegistry>,
        shutdown_rx: &mut watch::Receiver<bool>,
    ) {
        let interval = Duration::from_secs(HEALTH_CHECK_INTERVAL_SECS);

        loop {
            tokio::select! {
                () = tokio::time::sleep(interval) => {
                    // List triggers cleanup of stale entries
                    match registry.list() {
                        Ok(instances) => {
                            tracing::debug!(
                                "Health check: {} active instances",
                                instances.len()
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Health check failed: {e}");
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::debug!("Health check loop shutting down");
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_bind() {
        // Bind to port 0 for OS-assigned port
        let daemon = ManagerDaemon::bind_addr("127.0.0.1", 0).await.unwrap();
        let addr = daemon.local_addr().unwrap();
        assert!(addr.port() > 0);
    }

    #[tokio::test]
    async fn test_manager_handle_ping() {
        let registry = Arc::new(InstanceRegistry::new());
        let (shutdown_tx, _) = watch::channel(false);

        let request = ManagerRequest::new(1, ManagerMethod::Ping);
        let response = ManagerDaemon::handle_request(&request, &registry, &shutdown_tx);

        assert_eq!(response.id, 1);
        assert!(response.result.is_ok());
    }

    #[tokio::test]
    async fn test_manager_handle_list_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = Arc::new(InstanceRegistry::with_dir(temp_dir.path().to_path_buf()));
        let (shutdown_tx, _) = watch::channel(false);

        let request = ManagerRequest::new(1, ManagerMethod::List);
        let response = ManagerDaemon::handle_request(&request, &registry, &shutdown_tx);

        assert_eq!(response.id, 1);
        assert!(response.result.is_ok());
    }

    #[tokio::test]
    async fn test_manager_handle_query_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = Arc::new(InstanceRegistry::with_dir(temp_dir.path().to_path_buf()));
        let (shutdown_tx, _) = watch::channel(false);

        let request = ManagerRequest::new(
            1,
            ManagerMethod::Query {
                name: "nonexistent".to_string(),
            },
        );
        let response = ManagerDaemon::handle_request(&request, &registry, &shutdown_tx);

        assert_eq!(response.id, 1);
        assert!(response.result.is_error());
    }
}
