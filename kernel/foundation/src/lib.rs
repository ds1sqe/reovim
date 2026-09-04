#![no_std]
#![forbid(unsafe_code)]
//! Domain-neutral primitives for Reovim public contracts.
//!
//! Foundation owns only explicit scalar values and descriptors. It does not
//! select an environment, editing Domain, transport, Client, or adapter.

pub mod bound;
pub mod carrier;
pub mod contract;
pub mod identity;
