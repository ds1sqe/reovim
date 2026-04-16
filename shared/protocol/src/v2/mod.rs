//! Protocol v2: gRPC-based raw-data model.
//!
//! Unlike v1 (JSON-RPC with cell grids), v2 uses gRPC and provides
//! raw buffer data. Clients (TUI, GUI, Web) render their own views.
//!
//! # Philosophy
//!
//! - **Server = mechanism** (provides WHAT: buffer content, cursor, options)
//! - **Client = policy** (decides HOW: gutters, decorations, styling)
//!
//! The server has no concept of "screen" or "cell grid". It only knows about
//! buffers, windows (abstract containers), and editor state. How that gets
//! rendered is 100% client policy.
//!
//! # Services
//!
//! - [`InputService`] - Send keys to the editor
//! - [`StateService`] - Query editor state (mode, cursor, layout)
//! - [`BufferService`] - Access raw buffer content
//! - [`EditorService`] - Editor-level operations (resize, quit)
//! - [`ModuleService`] - Dynamic module management
//! - [`ServerService`] - Server management (kill, info)
//! - [`NotificationService`] - Server-to-client streaming
//!
//! # Usage
//!
//! ```ignore
//! // Server side
//! use reovim_protocol::v2::buffer_service_server::BufferServiceServer;
//!
//! // Client side
//! use reovim_protocol::v2::buffer_service_client::BufferServiceClient;
//! ```
//!
//! # Feature Flag
//!
//! This module requires the `grpc` feature:
//!
//! ```toml
//! reovim-protocol = { version = "0.9", features = ["grpc"] }
//! ```

// Generated code from tonic-build is wrapped in a submodule to isolate clippy allows
#[allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    clippy::missing_const_for_fn,
    clippy::doc_markdown,
    clippy::similar_names,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use
)]
mod proto {
    tonic::include_proto!("reovim.v3");
}

pub use proto::*;
