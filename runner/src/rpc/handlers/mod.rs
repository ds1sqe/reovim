//! RPC method handlers.
//!
//! This module contains handlers for each supported RPC method.
//! Handlers are functions that take `RpcContext` and params,
//! returning a boxed future with the result.

mod input;
mod server;
mod state;

pub use {
    input::input_keys,
    server::server_kill,
    state::{state_cursor, state_mode},
};
