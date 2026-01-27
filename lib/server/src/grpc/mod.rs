//! gRPC v2 protocol implementation.
//!
//! Provides gRPC service implementations that bridge
//! Protocol v2 types to the session/buffer system.

mod buffer;

pub use buffer::BufferServiceImpl;
