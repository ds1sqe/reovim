//! LSP transport layer with Content-Length framing.
//!
//! LSP uses a specific framing format:
//! ```text
//! Content-Length: <byte-length>\r\n
//! \r\n
//! <JSON message>
//! ```
//!
//! This module provides async read/write functions for this format.

use std::io;

use {
    tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    tracing::{debug, trace},
};

use crate::jsonrpc::Message;

/// Error type for transport operations.
#[derive(Debug)]
pub enum TransportError {
    /// I/O error.
    Io(io::Error),
    /// JSON parsing error.
    Json(serde_json::Error),
    /// Missing Content-Length header.
    MissingContentLength,
    /// Invalid Content-Length value.
    InvalidContentLength(String),
    /// Connection closed (EOF).
    Closed,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::MissingContentLength => write!(f, "Missing Content-Length header"),
            Self::InvalidContentLength(s) => write!(f, "Invalid Content-Length: {s}"),
            Self::Closed => write!(f, "Connection closed"),
        }
    }
}

impl std::error::Error for TransportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::MissingContentLength | Self::InvalidContentLength(_) | Self::Closed => None,
        }
    }
}

impl From<io::Error> for TransportError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for TransportError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Transport for LSP communication.
///
/// Handles Content-Length framing for LSP messages over stdio.
pub struct Transport;

impl Transport {
    /// Read a message from the reader.
    ///
    /// Parses the Content-Length header and reads exactly that many bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Content-Length header is missing or invalid
    /// - The message body cannot be read
    /// - The JSON cannot be parsed
    /// - The connection is closed (EOF)
    pub async fn recv<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Message, TransportError> {
        let mut content_length: Option<usize> = None;
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;

            if bytes_read == 0 {
                return Err(TransportError::Closed);
            }

            let trimmed = line.trim();

            // Empty line signals end of headers
            if trimmed.is_empty() {
                break;
            }

            // Parse Content-Length header
            if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                let value = value.trim();
                content_length = Some(
                    value
                        .parse()
                        .map_err(|_| TransportError::InvalidContentLength(value.to_string()))?,
                );
            }
            // Ignore other headers (like Content-Type)
        }

        let content_length = content_length.ok_or(TransportError::MissingContentLength)?;

        // Read exactly content_length bytes
        let mut buffer = vec![0u8; content_length];
        reader.read_exact(&mut buffer).await?;

        let json_str = String::from_utf8_lossy(&buffer);
        trace!(target: "reovim_lsp", len = content_length, "<- {}", json_str);

        let message: Message = serde_json::from_slice(&buffer)?;
        debug!(target: "reovim_lsp", "<- {:?}", message);

        Ok(message)
    }

    /// Send a message to the writer.
    ///
    /// Formats the message with Content-Length header.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The message cannot be serialized to JSON
    /// - The message cannot be written
    pub async fn send<W: AsyncWrite + Unpin>(
        writer: &mut W,
        message: &Message,
    ) -> Result<(), TransportError> {
        let json = serde_json::to_string(message)?;
        let content_length = json.len();

        debug!(target: "reovim_lsp", "-> {:?}", message);
        trace!(target: "reovim_lsp", len = content_length, "-> {}", json);

        // Write header + body
        let header = format!("Content-Length: {content_length}\r\n\r\n");
        writer.write_all(header.as_bytes()).await?;
        writer.write_all(json.as_bytes()).await?;
        writer.flush().await?;

        Ok(())
    }

    /// Send a request to the writer.
    ///
    /// Convenience method that wraps the request in a [`Message`].
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent.
    pub async fn send_request<W: AsyncWrite + Unpin>(
        writer: &mut W,
        request: &crate::jsonrpc::Request,
    ) -> Result<(), TransportError> {
        Self::send(writer, &Message::Request(request.clone())).await
    }

    /// Send a notification to the writer.
    ///
    /// Convenience method that wraps the notification in a [`Message`].
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent.
    pub async fn send_notification<W: AsyncWrite + Unpin>(
        writer: &mut W,
        notification: &crate::jsonrpc::Notification,
    ) -> Result<(), TransportError> {
        Self::send(writer, &Message::Notification(notification.clone())).await
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
