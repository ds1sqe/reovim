//! gRPC v2 protocol implementation.
//!
//! This module provides gRPC service implementations that bridge
//! Protocol v2 types to the internal session/buffer system.
//!
//! # Architecture
//!
//! ```text
//! gRPC Client (GUI/Web)
//!     │
//!     ├─→ BufferService::get_raw_content()
//!     │       │
//!     │       └─→ Bridge to SessionRegistry
//!     │               │
//!     │               └─→ Session::with_state() → Buffer content
//!     │
//!     └─→ Response<GetRawContentResponse>
//! ```
//!
//! # Philosophy
//!
//! - **Server = mechanism** (provides WHAT: buffer content, cursor, options)
//! - **Client = policy** (decides HOW: gutters, decorations, styling)
//!
//! The gRPC services return raw data. Clients are responsible for rendering.

mod buffer;

pub use buffer::BufferServiceImpl;
