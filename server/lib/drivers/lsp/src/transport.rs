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
mod tests {
    use {
        super::*,
        crate::jsonrpc::{Notification, Request, Response},
        serde_json::Value,
        std::io::Cursor,
    };

    #[tokio::test]
    async fn test_recv_request() {
        let input =
            b"Content-Length: 54\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"test\",\"params\":null}";
        let mut reader = Cursor::new(input);

        let message = Transport::recv(&mut reader).await.unwrap();
        assert!(message.is_request());

        if let Message::Request(req) = message {
            assert_eq!(req.method, "test");
            assert_eq!(req.id, crate::jsonrpc::Id::Number(1));
        }
    }

    #[tokio::test]
    async fn test_recv_notification() {
        let input = b"Content-Length: 40\r\n\r\n{\"jsonrpc\":\"2.0\",\"method\":\"initialized\"}";
        let mut reader = Cursor::new(input);

        let message = Transport::recv(&mut reader).await.unwrap();
        assert!(message.is_notification());
    }

    #[tokio::test]
    async fn test_send_request() {
        let mut buffer = Vec::new();
        let request = Request::new(1_i64, "test", None);

        Transport::send_request(&mut buffer, &request)
            .await
            .unwrap();

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.starts_with("Content-Length:"));
        assert!(output.contains("\"method\":\"test\""));
    }

    #[tokio::test]
    async fn test_send_notification() {
        let mut buffer = Vec::new();
        let notification = Notification::new("initialized", None);

        Transport::send_notification(&mut buffer, &notification)
            .await
            .unwrap();

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.starts_with("Content-Length:"));
        assert!(output.contains("\"method\":\"initialized\""));
        assert!(!output.contains("\"id\""));
    }

    #[tokio::test]
    async fn test_recv_missing_content_length() {
        let input = b"\r\n{\"jsonrpc\":\"2.0\",\"method\":\"test\"}";
        let mut reader = Cursor::new(input);

        let result = Transport::recv(&mut reader).await;
        assert!(matches!(result, Err(TransportError::MissingContentLength)));
    }

    #[tokio::test]
    async fn test_recv_invalid_content_length() {
        let input = b"Content-Length: abc\r\n\r\n{}";
        let mut reader = Cursor::new(input);

        let result = Transport::recv(&mut reader).await;
        assert!(matches!(result, Err(TransportError::InvalidContentLength(_))));
    }

    #[tokio::test]
    async fn test_recv_with_extra_headers() {
        // LSP spec allows Content-Type header which we should ignore
        let input = b"Content-Length: 40\r\nContent-Type: application/json\r\n\r\n{\"jsonrpc\":\"2.0\",\"method\":\"initialized\"}";
        let mut reader = Cursor::new(input);

        let message = Transport::recv(&mut reader).await.unwrap();
        assert!(message.is_notification());
    }

    #[tokio::test]
    async fn test_recv_closed() {
        let input = b"";
        let mut reader = Cursor::new(input);

        let result = Transport::recv(&mut reader).await;
        assert!(matches!(result, Err(TransportError::Closed)));
    }

    #[tokio::test]
    async fn test_round_trip() {
        // Send a message, then receive it back
        let mut buffer = Vec::new();
        let request = Request::new(42_i64, "textDocument/hover", None);

        Transport::send_request(&mut buffer, &request)
            .await
            .unwrap();

        let mut reader = Cursor::new(buffer);
        let received = Transport::recv(&mut reader).await.unwrap();

        assert!(received.is_request());
        if let Message::Request(req) = received {
            assert_eq!(req.method, "textDocument/hover");
            assert_eq!(req.id, crate::jsonrpc::Id::Number(42));
        }
    }

    #[tokio::test]
    async fn test_round_trip_response() {
        let mut buffer = Vec::new();
        let response = Response::success(1_i64, Value::Null);
        let message = Message::Response(response);

        Transport::send(&mut buffer, &message).await.unwrap();

        let mut reader = Cursor::new(buffer);
        let received = Transport::recv(&mut reader).await.unwrap();
        assert!(received.is_response());
    }

    #[test]
    fn test_transport_error_display() {
        let io_err = TransportError::Io(io::Error::new(io::ErrorKind::BrokenPipe, "broken"));
        assert!(format!("{io_err}").contains("broken"));

        let json_err = TransportError::Json(serde_json::from_str::<Value>("invalid").unwrap_err());
        assert!(format!("{json_err}").contains("JSON error"));

        let missing = TransportError::MissingContentLength;
        assert_eq!(format!("{missing}"), "Missing Content-Length header");

        let invalid = TransportError::InvalidContentLength("xyz".to_string());
        assert!(format!("{invalid}").contains("xyz"));

        let closed = TransportError::Closed;
        assert_eq!(format!("{closed}"), "Connection closed");
    }

    #[test]
    fn test_transport_error_source() {
        use std::error::Error;

        let io_err = TransportError::Io(io::Error::other("test"));
        assert!(io_err.source().is_some());

        let json_err = TransportError::Json(serde_json::from_str::<Value>("invalid").unwrap_err());
        assert!(json_err.source().is_some());

        let missing = TransportError::MissingContentLength;
        assert!(missing.source().is_none());

        let invalid = TransportError::InvalidContentLength("abc".to_string());
        assert!(invalid.source().is_none());

        let closed = TransportError::Closed;
        assert!(closed.source().is_none());
    }

    #[test]
    fn test_transport_error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "test");
        let transport_err: TransportError = io_err.into();
        assert!(matches!(transport_err, TransportError::Io(_)));
    }

    #[test]
    fn test_transport_error_from_json() {
        let json_err = serde_json::from_str::<Value>("invalid").unwrap_err();
        let transport_err: TransportError = json_err.into();
        assert!(matches!(transport_err, TransportError::Json(_)));
    }
}
