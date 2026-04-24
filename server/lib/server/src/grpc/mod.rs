//! gRPC v2 protocol implementation.
//!
//! Provides gRPC service implementations that bridge
//! Protocol v2 types to the session/buffer system.
//!
//! # Architecture
//!
//! The gRPC services follow the data/presentation separation model:
//! - Server provides raw data (buffer content, mode, cursor position)
//! - Client handles presentation (colors, layout, rendering)
//!
//! Some services are stubs in `lib/server` because full implementation
//! requires application-level concerns (e.g., module loading belongs in runner).
//!
//! See `docs/architecture/server-client-split.md` for details.

pub mod auth;
mod buffer;
mod client_debug;
mod command;
mod debug;
mod editor;
mod extension;
mod input;
mod module;
mod notification;
pub mod notification_builder;
mod presence;
mod server_service;
mod state;

pub use {
    auth::AuthInterceptor, buffer::BufferServiceImpl, client_debug::ClientDebugServiceImpl,
    command::CommandServiceImpl, debug::DebugServiceImpl, editor::EditorServiceImpl,
    extension::ExtensionServiceImpl, input::InputServiceImpl, module::ModuleServiceImpl,
    notification::NotificationServiceImpl, presence::PresenceServiceImpl,
    server_service::ServerServiceImpl, state::StateServiceImpl,
};
