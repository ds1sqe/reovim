//! Copied namespace observations; names grant and retain no subjects.

use crate::ResourceId;
use alloc::boxed::Box;

/// One owned name/identity observation, e.g. a listed module contribution.
/// Names follow namespace byte policy, independently of Resource contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceEntry {
    /// Owned child name bytes, e.g. `b"counter"`.
    pub name: Box<[u8]>,
    /// Observed identity; acquire it separately under caller grants.
    pub resource: ResourceId,
}
