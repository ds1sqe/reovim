//! Protocol v3: gRPC-based domain-neutral model (#753).
//!
//! Domain-specific data flows as opaque `DomainDatum` bytes. Server never
//! decodes domain content — pure passthrough. Domain drivers encode, client
//! domain view modules decode via codec crates.
//!
//! # Services
//!
//! - [`InputService`] - Send opaque input events
//! - [`StateService`] - Query projections, options, layout
//! - [`BufferService`] - Buffer metadata and lifecycle
//! - [`EditorService`] - Surface change, quit, active buffer
//! - [`PresenceService`] - Multi-client awareness with spatial state
//! - [`NotificationService`] - Server-to-client streaming
//! - [`DebugService`] - Debug tools (log, capture, projections)
//! - [`ModuleService`] - Dynamic module management
//! - [`ServerService`] - Server management
//! - [`CommandService`] - Command completion
//! - [`ExtensionService`] - Extension state queries

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
