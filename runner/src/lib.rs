//! Reovim Runner - Headless Editor Server
//!
//! This crate provides a headless editor server using the kernel-driver
//! type system from issue #213. It implements the "mechanism vs policy"
//! principle where:
//!
//! - **Mechanism** (this crate): Server, sessions, registries
//! - **Policy** (modules): Mode implementations, commands, keybindings
//!
//! # Architecture
//!
//! ```text
//! runner/
//! ├── app.rs           - AppState (kernel + runtime state)
//! ├── server.rs        - Server and ServerConfig (main entry)
//! ├── fallback.rs      - InputFallbackHandler trait (mechanism)
//! ├── event_loop.rs    - Synchronous event loop (for embedded use)
//! ├── session/
//! │   ├── id.rs        - SessionId, ClientId types
//! │   ├── state.rs     - SessionState (AppState + Registries)
//! │   ├── session.rs   - Session (async-safe state access)
//! │   └── registry.rs  - SessionRegistry (lock-free)
//! ├── client/
//! │   ├── client.rs    - Client (per-connection state)
//! │   └── registry.rs  - ClientRegistry (per-session)
//! ├── transport/
//! │   └── tcp.rs       - TCP transport with port fallback
//! ├── rpc/
//! │   ├── dispatcher.rs - RPC method routing
//! │   └── handlers/     - Method handlers (input, state, server)
//! └── registry/
//!     ├── mode.rs      - ModeRegistry
//!     ├── command.rs   - CommandRegistry
//!     └── keymap.rs    - KeymapRegistry
//! ```
//!
//! # Example - Server Mode
//!
//! ```ignore
//! use runner::session::{Session, SessionId, SessionRegistry};
//! use reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId};
//!
//! // Create session registry
//! let registry = SessionRegistry::new();
//!
//! // Create and register a session
//! let session = Session::new(
//!     SessionId::new("default"),
//!     KernelContext::default(),
//!     ModeId::new(ModuleId::new("editor"), "normal"),
//! );
//! registry.insert(session);
//!
//! // Lock-free session lookup
//! if let Some(session) = registry.get(&SessionId::default()) {
//!     let mode = session.current_mode().await;
//! }
//! ```
//!
//! # Example - Embedded Mode (Synchronous)
//!
//! ```ignore
//! use runner::{AppState, EventLoop, registry::*};
//! use runner::fallback::NoOpFallback;
//!
//! let app = AppState::new(kernel_context, initial_mode);
//! let mut event_loop = EventLoop::new(
//!     app,
//!     mode_registry,
//!     command_registry,
//!     keymap_registry,
//!     NoOpFallback,
//! );
//! event_loop.run()?;
//! ```

mod app;
pub mod client;
mod event_loop;
mod fallback;
pub mod notification;
pub mod registry;
pub mod rpc;
pub mod server;
pub mod session;
pub mod transport;

pub use {
    app::AppState,
    event_loop::{EventLoop, EventLoopError},
    fallback::{BeepFallback, FallbackResult, InputFallbackHandler, NoOpFallback},
    notification::NotificationBroadcaster,
    server::{Server, ServerConfig, TransportMode},
};
