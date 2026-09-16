#![no_std]
#![deny(unsafe_code)]
//! Common Resource access and executable provider contracts.

extern crate alloc;

#[allow(unsafe_code)]
pub mod abi;
pub mod call;
pub mod caller;
pub mod declaration;
pub mod driver;
pub mod error;
pub mod module;
pub mod namespace;
pub mod notify;
pub mod provider;
pub mod resource;

pub use call::{Outcome, Pending, Value};
pub use error::{Error, ErrorCode};
pub use resource::ResourceId;
