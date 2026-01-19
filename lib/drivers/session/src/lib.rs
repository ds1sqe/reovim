//! Session driver for reovim.
//!
//! Provides traits and types for session management.
//!
//! # Empty Session Handling
//!
//! The [`EmptySessionHandler`] trait defines how to handle sessions
//! with no buffers. Modules implement this to define policy (e.g.,
//! create a scratch buffer, show a welcome screen).
//!
//! ```ignore
//! use reovim_driver_session::{
//!     EmptySessionHandler, EmptySessionContext, EmptySessionAction
//! };
//!
//! struct MyHandler;
//!
//! impl EmptySessionHandler for MyHandler {
//!     fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction {
//!         EmptySessionAction::CreateBuffer {
//!             name: None,
//!             content: String::new(),
//!         }
//!     }
//!     fn id(&self) -> &'static str { "my-module:handler" }
//!     fn description(&self) -> &'static str { "My handler" }
//! }
//! ```

mod empty_handler;

pub use empty_handler::{EmptySessionAction, EmptySessionContext, EmptySessionHandler};
