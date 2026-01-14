//! Transport connection types for TCP, Unix, and Stdio.
//!
//! Provides `TransportReader` and `TransportWriter` abstractions that unify
//! different transport mechanisms (TCP, Unix socket, Stdio) behind a common interface.
//!
//! # Design
//!
//! Uses enum-based polymorphism rather than trait objects for:
//! - Zero-cost abstractions (no dynamic dispatch)
//! - Simpler lifetime management
//! - Better compile-time optimization
//!
//! # Thread Safety
//!
//! - `TransportReader`: NOT `Send` for Stdio (`tokio::io::Stdin` is !Send)
//! - `TransportWriter`: Uses `tokio::sync::Mutex` for thread-safe concurrent writes

use std::io;

#[cfg(unix)]
use tokio::net::unix::{OwnedReadHalf as UnixReadHalf, OwnedWriteHalf as UnixWriteHalf};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    sync::Mutex,
};

/// Reader that abstracts TCP, Unix socket, and Stdio input.
///
/// Provides line-based reading suitable for JSON-RPC over newline-delimited JSON.
pub struct TransportReader {
    inner: ReaderInner,
}

enum ReaderInner {
    Tcp(BufReader<OwnedReadHalf>),
    #[cfg(unix)]
    Unix(BufReader<UnixReadHalf>),
    Stdio(BufReader<tokio::io::Stdin>),
}

impl TransportReader {
    /// Create a reader from a TCP connection.
    #[must_use]
    pub fn from_tcp(read_half: OwnedReadHalf) -> Self {
        Self {
            inner: ReaderInner::Tcp(BufReader::new(read_half)),
        }
    }

    /// Create a reader from a Unix socket connection.
    #[cfg(unix)]
    #[must_use]
    pub fn from_unix(read_half: UnixReadHalf) -> Self {
        Self {
            inner: ReaderInner::Unix(BufReader::new(read_half)),
        }
    }

    /// Create a reader from stdin.
    #[must_use]
    pub fn from_stdio() -> Self {
        Self {
            inner: ReaderInner::Stdio(BufReader::new(tokio::io::stdin())),
        }
    }

    /// Read a line from the transport.
    ///
    /// Returns the line without the trailing newline character.
    /// Returns `Ok(None)` on EOF.
    ///
    /// # Errors
    ///
    /// Returns an error if reading fails.
    pub async fn read_line(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        let bytes_read = match &mut self.inner {
            ReaderInner::Tcp(reader) => reader.read_line(&mut line).await?,
            #[cfg(unix)]
            ReaderInner::Unix(reader) => reader.read_line(&mut line).await?,
            ReaderInner::Stdio(reader) => reader.read_line(&mut line).await?,
        };

        if bytes_read == 0 {
            return Ok(None);
        }

        // Remove trailing newline
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        Ok(Some(line))
    }
}

/// Writer that abstracts TCP, Unix socket, and Stdio output.
///
/// Thread-safe via internal `Mutex`, allowing concurrent writes from
/// multiple tasks (e.g., response sending and notification broadcasting).
pub struct TransportWriter {
    inner: Mutex<WriterInner>,
}

enum WriterInner {
    Tcp(BufWriter<OwnedWriteHalf>),
    #[cfg(unix)]
    Unix(BufWriter<UnixWriteHalf>),
    Stdio(BufWriter<tokio::io::Stdout>),
}

impl TransportWriter {
    /// Create a writer from a TCP connection.
    #[must_use]
    pub fn from_tcp(write_half: OwnedWriteHalf) -> Self {
        Self {
            inner: Mutex::new(WriterInner::Tcp(BufWriter::new(write_half))),
        }
    }

    /// Create a writer from a Unix socket connection.
    #[cfg(unix)]
    #[must_use]
    pub fn from_unix(write_half: UnixWriteHalf) -> Self {
        Self {
            inner: Mutex::new(WriterInner::Unix(BufWriter::new(write_half))),
        }
    }

    /// Create a writer to stdout.
    #[must_use]
    pub fn from_stdio() -> Self {
        Self {
            inner: Mutex::new(WriterInner::Stdio(BufWriter::new(tokio::io::stdout()))),
        }
    }

    /// Write a line to the transport.
    ///
    /// Appends a newline character and flushes the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing or flushing fails.
    pub async fn write_line(&self, line: &str) -> io::Result<()> {
        let mut guard = self.inner.lock().await;
        match &mut *guard {
            WriterInner::Tcp(writer) => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await
            }
            #[cfg(unix)]
            WriterInner::Unix(writer) => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await
            }
            WriterInner::Stdio(writer) => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await
            }
        }
    }
}

impl std::fmt::Debug for TransportReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variant = match &self.inner {
            ReaderInner::Tcp(_) => "Tcp",
            #[cfg(unix)]
            ReaderInner::Unix(_) => "Unix",
            ReaderInner::Stdio(_) => "Stdio",
        };
        f.debug_struct("TransportReader")
            .field("type", &variant)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for TransportWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransportWriter").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_reader_debug() {
        // We can't easily create a reader without a real connection,
        // but we can test the Debug impl structure exists
    }

    #[test]
    fn test_transport_writer_debug() {
        let writer = TransportWriter::from_stdio();
        let debug_str = format!("{writer:?}");
        assert!(debug_str.contains("TransportWriter"));
    }

    #[tokio::test]
    async fn test_stdio_writer_creation() {
        // Verify we can create a stdio writer without panicking
        let _writer = TransportWriter::from_stdio();
    }

    #[tokio::test]
    async fn test_stdio_reader_creation() {
        // Verify we can create a stdio reader without panicking
        let _reader = TransportReader::from_stdio();
    }
}
