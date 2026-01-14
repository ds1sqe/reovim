//! LSP client library for reovim text editor.
//!
//! This crate provides a Language Server Protocol client implementation
//! following reovim's saturator pattern for non-blocking I/O.
//!
//! # Architecture
//!
//! The LSP client uses a background task (saturator) pattern:
//! - `LspSaturator` runs in a `tokio::spawn` task and owns the LSP client
//! - `DiagnosticCache` uses `ArcSwap` for lock-free reads from render thread
//! - `mpsc::channel(1)` with `try_send()` provides non-blocking requests
//!
//! This ensures the render thread never blocks on LSP I/O.

mod cache;
mod client;
mod jsonrpc;
mod progress;
mod progress_event;
mod saturator;
mod transport;

pub use {
    cache::{BufferDiagnostics, DiagnosticCache},
    client::{Client, ClientConfig, ClientError, uri_from_path},
    jsonrpc::{Error as JsonRpcError, Id, Message, Notification, Request, Response},
    progress::{
        ProgressParams, ProgressToken, WorkDoneProgressBegin, WorkDoneProgressEnd,
        WorkDoneProgressReport, WorkDoneProgressValue,
    },
    progress_event::{LspProgressBegin, LspProgressEnd, LspProgressReport},
    saturator::{LspRequest, LspSaturator, LspSaturatorHandle},
    transport::Transport,
};

// Re-export commonly used lsp-types
pub use lsp_types::{
    Diagnostic, DiagnosticSeverity, GotoDefinitionResponse, Hover, HoverContents, InitializeParams,
    InitializeResult, Location, MarkedString, MarkupContent, MarkupKind, Position, Range,
    ServerCapabilities, TextDocumentIdentifier, TextDocumentPositionParams, Uri,
};

// Re-export url::Url for file:// URI handling
pub use url::Url;
