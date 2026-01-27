//! gRPC v2 protocol implementation.
//!
//! Provides gRPC service implementations that bridge
//! Protocol v2 types to the session/buffer system.

mod buffer;
mod input;
mod notification;
mod server_service;
mod state;

pub use {
    buffer::BufferServiceImpl, input::InputServiceImpl, notification::NotificationServiceImpl,
    server_service::ServerServiceImpl, state::StateServiceImpl,
};
