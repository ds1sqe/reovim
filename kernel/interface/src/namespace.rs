//! Owned namespace observations; directory entries and indexes stay private.

use alloc::boxed::Box;

use crate::resource::ResourceId;

/// One name from a coherent, sorted listing in the caller's granted scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceEntry {
    /// Detached name; changing it cannot rename the subject.
    pub name: Box<str>,
    /// Connection-scoped subject, without a lease or added authority.
    pub resource: ResourceId,
}
