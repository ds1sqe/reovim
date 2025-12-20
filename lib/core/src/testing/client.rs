//! Test client for server-based integration tests
//!
//! Provides a simple API for interacting with a reovim server via JSON-RPC.

use {
    serde::{Deserialize, Serialize},
    serde_json::{Value, json},
    std::io,
    tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
        net::TcpStream,
    },
};

/// Error type for test client operations
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

/// Mode information returned by the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeInfo {
    pub display: String,
    pub edit_mode: String,
    pub focus: String,
    pub sub_mode: String,
}

/// Telescope state information returned by the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelescopeInfo {
    pub active: bool,
    pub query: String,
    pub selected_index: usize,
    pub item_count: usize,
    pub picker_name: String,
    pub title: String,
    pub selected_item: Option<String>,
}

/// Screen state information returned by the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenInfo {
    pub width: u16,
    pub height: u16,
    pub active_buffer_id: usize,
    pub active_window_id: Option<usize>,
    pub window_count: usize,
}

/// Window state information returned by the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: usize,
    pub buffer_id: usize,
    pub buffer_anchor_x: u16,
    pub buffer_anchor_y: u16,
    pub is_active: bool,
    pub cursor_x: u16,
    pub cursor_y: u16,
}

/// Buffer information returned by buffer/list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferListItem {
    pub id: usize,
    pub name: String,
    pub modified: bool,
}

/// Test client for server-based integration tests
pub struct TestClient {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: BufWriter<tokio::net::tcp::OwnedWriteHalf>,
    next_id: u64,
}

impl TestClient {
    /// Connect to a reovim server via TCP
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn connect(host: &str, port: u16) -> Result<Self, ClientError> {
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(ClientError::ConnectFailed)?;
        let (read_half, write_half) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(read_half),
            writer: BufWriter::new(write_half),
            next_id: 1,
        })
    }

    /// Send an RPC request and wait for the response
    async fn send(&mut self, method: &str, params: Value) -> Result<Value, ClientError> {
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
        self.writer
            .write_all(request_str.as_bytes())
            .await
            .map_err(ClientError::WriteFailed)?;
        self.writer
            .write_all(b"\n")
            .await
            .map_err(ClientError::WriteFailed)?;
        self.writer
            .flush()
            .await
            .map_err(ClientError::WriteFailed)?;

        // Read response (may need to skip notifications)
        loop {
            let mut line = String::new();
            let bytes_read = self
                .reader
                .read_line(&mut line)
                .await
                .map_err(ClientError::ReadFailed)?;

            if bytes_read == 0 {
                return Err(ClientError::ConnectionClosed);
            }

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

    /// Inject key sequence (vim notation)
    ///
    /// Returns the number of keys injected.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn keys(&mut self, keys: &str) -> Result<u64, ClientError> {
        let result = self.send("input/keys", json!({ "keys": keys })).await?;
        Ok(result["injected"].as_u64().unwrap_or(0))
    }

    /// Get current mode
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn mode(&mut self) -> Result<ModeInfo, ClientError> {
        let result = self.send("state/mode", json!({})).await?;
        serde_json::from_value(result).map_err(ClientError::ParseFailed)
    }

    /// Get cursor position
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    #[allow(clippy::cast_possible_truncation)]
    pub async fn cursor(&mut self) -> Result<(u16, u16), ClientError> {
        let result = self.send("state/cursor", json!({})).await?;
        let x = result["x"].as_u64().unwrap_or(0) as u16;
        let y = result["y"].as_u64().unwrap_or(0) as u16;
        Ok((x, y))
    }

    /// Get buffer content
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn buffer_content(&mut self) -> Result<String, ClientError> {
        let result = self.send("buffer/get_content", json!({})).await?;
        Ok(result["content"].as_str().unwrap_or("").to_string())
    }

    /// Set buffer content
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn set_buffer_content(&mut self, content: &str) -> Result<(), ClientError> {
        self.send("buffer/set_content", json!({ "content": content }))
            .await?;
        Ok(())
    }

    /// Kill the server
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn kill(&mut self) -> Result<(), ClientError> {
        self.send("server/kill", json!({})).await?;
        Ok(())
    }

    /// Resize the editor
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn resize(&mut self, width: u16, height: u16) -> Result<(), ClientError> {
        self.send("editor/resize", json!({ "width": width, "height": height }))
            .await?;
        Ok(())
    }

    /// Quit the editor
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn quit(&mut self) -> Result<(), ClientError> {
        self.send("editor/quit", json!({})).await?;
        Ok(())
    }

    /// Get telescope state
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn telescope(&mut self) -> Result<TelescopeInfo, ClientError> {
        let result = self.send("state/telescope", json!({})).await?;
        serde_json::from_value(result).map_err(ClientError::ParseFailed)
    }

    /// Get screen content with raw ANSI escape codes
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn screen_content_raw(&mut self) -> Result<String, ClientError> {
        let result = self
            .send("state/screen_content", json!({ "format": "raw_ansi" }))
            .await?;
        Ok(result["content"].as_str().unwrap_or("").to_string())
    }

    /// Get screen state (dimensions and active buffer)
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn screen(&mut self) -> Result<ScreenInfo, ClientError> {
        let result = self.send("state/screen", json!({})).await?;
        serde_json::from_value(result).map_err(ClientError::ParseFailed)
    }

    /// Get all windows state (scroll position, cursor, active state)
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn windows(&mut self) -> Result<Vec<WindowInfo>, ClientError> {
        let result = self.send("state/windows", json!({})).await?;
        serde_json::from_value(result).map_err(ClientError::ParseFailed)
    }

    /// Get list of all buffers
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn buffer_list(&mut self) -> Result<Vec<BufferListItem>, ClientError> {
        let result = self.send("buffer/list", json!({})).await?;
        serde_json::from_value(result["buffers"].clone()).map_err(ClientError::ParseFailed)
    }

    /// Open a file in a new buffer
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    #[allow(clippy::cast_possible_truncation)]
    pub async fn open_file(&mut self, path: &str) -> Result<usize, ClientError> {
        let result = self
            .send("buffer/open_file", json!({ "path": path }))
            .await?;
        Ok(result["buffer_id"].as_u64().unwrap_or(0) as usize)
    }

    // === Visual methods ===

    /// Get visual snapshot of the screen
    ///
    /// Returns a structured snapshot with cell grid, cursor, and layer info.
    /// Useful for debugging and programmatic assertions.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or frame renderer is not enabled.
    pub async fn visual_snapshot(&mut self) -> Result<crate::visual::VisualSnapshot, ClientError> {
        let result = self.send("state/visual_snapshot", json!({})).await?;
        serde_json::from_value(result).map_err(ClientError::ParseFailed)
    }

    /// Get ASCII art representation of the screen
    ///
    /// # Arguments
    ///
    /// * `annotated` - If true, includes borders and row/column numbers
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or frame renderer is not enabled.
    pub async fn ascii_art(&mut self, annotated: bool) -> Result<String, ClientError> {
        let result = self
            .send("state/ascii_art", json!({ "annotated": annotated }))
            .await?;
        Ok(result["content"].as_str().unwrap_or("").to_string())
    }

    /// Get layer visibility information
    ///
    /// Returns information about all layers including their z-order and bounds.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails.
    pub async fn layer_info(&mut self) -> Result<Vec<crate::visual::LayerInfo>, ClientError> {
        let result = self.send("state/layer_info", json!({})).await?;
        serde_json::from_value(result).map_err(ClientError::ParseFailed)
    }
}
