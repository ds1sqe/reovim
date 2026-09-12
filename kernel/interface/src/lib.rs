#![no_std]
#![deny(unsafe_code)]
//! Common Resource access and executable provider contracts.

extern crate alloc;

#[allow(unsafe_code)]
pub mod abi;
pub mod access;
pub mod buffer;
pub mod driver;
pub mod error;
pub mod module;
pub mod namespace;
pub mod notify;
pub mod provider;
pub mod requests;
pub mod resource;
pub mod session;

pub use access::{Connection, Contract, Message, RequestError};
pub use error::{Error, ErrorCode};
pub use resource::ResourceId;
