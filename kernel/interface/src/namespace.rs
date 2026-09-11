use alloc::boxed::Box;

use crate::ResourceId;

/// An owned observation; directory entries and indexes stay private.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceEntry {
    pub name: Box<str>,
    pub resource: ResourceId,
}
