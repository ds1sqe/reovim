//! RPC client for communicating with reovim server
//!
//! Connects to an existing reovim server via Unix socket or TCP.

use {
    serde_json::{Value, json},
    std::{io, path::Path},
    tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
        net::{TcpStream, UnixStream},
    },
};

/// Connection configuration
#[derive(Debug, Clone)]
pub enum ConnectionConfig {
    /// Connect via Unix socket
    UnixSocket(String),
    /// Connect via TCP
    Tcp { host: String, port: u16 },
}

/// Error type for RPC client operations
#[derive(Debug)]
pub enum ClientError {
    /// Failed to connect to the server
    ConnectFailed(io::Error),
    /// Failed to write to server
    WriteFailed(io::Error),
    /// Failed to read from server
    ReadFailed(io::Error),
    /// Failed to parse JSON response
    ParseFailed(serde_json::Error),
    /// Server returned an error response
    RpcError { code: i64, message: String },
    /// Server connection closed unexpectedly
    ConnectionClosed,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectFailed(e) => write!(f, "Failed to connect: {e}"),
            Self::WriteFailed(e) => write!(f, "Failed to write to server: {e}"),
            Self::ReadFailed(e) => write!(f, "Failed to read from server: {e}"),
            Self::ParseFailed(e) => write!(f, "Failed to parse response: {e}"),
            Self::RpcError { code, message } => {
                write!(f, "RPC error ({code}): {message}")
            }
            Self::ConnectionClosed => write!(f, "Server connection closed"),
        }
    }
}

impl std::error::Error for ClientError {}

/// Transport abstraction for the client
enum Transport {
    UnixSocket {
        reader: BufReader<tokio::net::unix::OwnedReadHalf>,
        writer: BufWriter<tokio::net::unix::OwnedWriteHalf>,
    },
    Tcp {
        reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
        writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    },
}

/// RPC client that communicates with reovim server
pub struct ReoClient {
    transport: Transport,
    next_id: u64,
}

impl ReoClient {
    /// Connect to a reovim server via Unix socket
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect_unix(path: impl AsRef<Path>) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(path)
            .await
            .map_err(ClientError::ConnectFailed)?;
        let (read_half, write_half) = stream.into_split();
        Ok(Self {
            transport: Transport::UnixSocket {
                reader: BufReader::new(read_half),
                writer: BufWriter::new(write_half),
            },
            next_id: 1,
        })
    }

    /// Connect to a reovim server via TCP
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect_tcp(host: &str, port: u16) -> Result<Self, ClientError> {
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(ClientError::ConnectFailed)?;
        let (read_half, write_half) = stream.into_split();
        Ok(Self {
            transport: Transport::Tcp {
                reader: BufReader::new(read_half),
                writer: BufWriter::new(write_half),
            },
            next_id: 1,
        })
    }

    /// Connect using the given configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect(config: &ConnectionConfig) -> Result<Self, ClientError> {
        match config {
            ConnectionConfig::UnixSocket(path) => Self::connect_unix(path).await,
            ConnectionConfig::Tcp { host, port } => Self::connect_tcp(host, *port).await,
        }
    }

    /// Send an RPC request and wait for the response
    ///
    /// # Errors
    ///
    /// Returns an error if sending or receiving fails, or if the server returns an error.
    pub async fn send(&mut self, method: &str, params: Value) -> Result<Value, ClientError> {
        let id = self.next_id;
        self.next_id += 1;

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        // Send request
        let request_str = serde_json::to_string(&request).map_err(ClientError::ParseFailed)?;
        self.write_line(&request_str).await?;

        // Read response (may need to skip notifications)
        loop {
            let line = self.read_line().await?;
            let response: Value =
                serde_json::from_str(line.trim()).map_err(ClientError::ParseFailed)?;

            // Check if this is our response (has matching id)
            if let Some(response_id) = response.get("id")
                && response_id == &json!(id)
            {
                // Check for error
                if let Some(error) = response.get("error") {
                    let code = error.get("code").and_then(Value::as_i64).unwrap_or(-1);
                    let message = error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Unknown error")
                        .to_string();
                    return Err(ClientError::RpcError { code, message });
                }

                // Return result
                return Ok(response.get("result").cloned().unwrap_or(Value::Null));
            }
            // If no id or wrong id, it's a notification - continue reading
        }
    }

    /// Send a raw JSON request string
    ///
    /// # Errors
    ///
    /// Returns an error if sending or receiving fails.
    pub async fn send_raw(&mut self, json_str: &str) -> Result<Value, ClientError> {
        // Parse to validate and extract id
        let request: Value = serde_json::from_str(json_str).map_err(ClientError::ParseFailed)?;

        let id = request
            .get("id")
            .cloned()
            .unwrap_or_else(|| json!(self.next_id));

        // Send request
        self.write_line(json_str).await?;

        // Read response
        loop {
            let line = self.read_line().await?;
            let response: Value =
                serde_json::from_str(line.trim()).map_err(ClientError::ParseFailed)?;

            if let Some(response_id) = response.get("id")
                && response_id == &id
            {
                if let Some(error) = response.get("error") {
                    let code = error.get("code").and_then(Value::as_i64).unwrap_or(-1);
                    let message = error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Unknown error")
                        .to_string();
                    return Err(ClientError::RpcError { code, message });
                }
                return Ok(response.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }

    async fn write_line(&mut self, line: &str) -> Result<(), ClientError> {
        match &mut self.transport {
            Transport::UnixSocket { writer, .. } => {
                writer
                    .write_all(line.as_bytes())
                    .await
                    .map_err(ClientError::WriteFailed)?;
                writer
                    .write_all(b"\n")
                    .await
                    .map_err(ClientError::WriteFailed)?;
                writer.flush().await.map_err(ClientError::WriteFailed)?;
            }
            Transport::Tcp { writer, .. } => {
                writer
                    .write_all(line.as_bytes())
                    .await
                    .map_err(ClientError::WriteFailed)?;
                writer
                    .write_all(b"\n")
                    .await
                    .map_err(ClientError::WriteFailed)?;
                writer.flush().await.map_err(ClientError::WriteFailed)?;
            }
        }
        Ok(())
    }

    async fn read_line(&mut self) -> Result<String, ClientError> {
        let mut line = String::new();
        let bytes_read = match &mut self.transport {
            Transport::UnixSocket { reader, .. } => reader
                .read_line(&mut line)
                .await
                .map_err(ClientError::ReadFailed)?,
            Transport::Tcp { reader, .. } => reader
                .read_line(&mut line)
                .await
                .map_err(ClientError::ReadFailed)?,
        };

        if bytes_read == 0 {
            return Err(ClientError::ConnectionClosed);
        }

        Ok(line)
    }
}
