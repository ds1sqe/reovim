//! Provider-owned names and stable opaque object keys.

use super::Creation;
use crate::{Error, Pending};

/// Independently offered namespace operations, e.g. a lookup-only mount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent optional operations, not mutually exclusive states"
)]
pub struct NamespaceInfo {
    /// Lookup returns canonical Bytes containing the object's stable key.
    pub lookup: bool,
    /// Creates through the default factory with a staged namespace key.
    pub create: bool,
    /// List returns name/key pairs and an opaque continuation cursor.
    pub list: bool,
    /// Materialization stages an implementation for an existing object key.
    pub materialize: bool,
    /// Removes one name after revalidating its captured object key.
    pub remove: bool,
    /// Adds an exclusive name for a retained key in this mounted namespace.
    pub link: bool,
    /// Moves one name atomically into an absent destination.
    pub rename: bool,
}

/// Lookup borrows one child name only until the callback returns.
/// A passive lookup reports zero activity, e.g. resolving a device name.
pub type LookupFn<State> = fn(&mut State, &[u8]) -> Result<Pending, Error>;
/// List returns a bounded canonical page: List<Record{name:Bytes,key:Bytes}>
/// followed by U64 next cursor; zero next ends the enumeration.
pub type ListFn<State> = fn(&mut State, u64, u64) -> Result<Pending, Error>;
/// Materialize revalidates the key, stages one primary subject and returns
/// Unit.
/// It does not create a new external object or confer caller access.
pub type MaterializeFn<State> =
    fn(&mut State, &[u8], &Creation) -> Result<Pending, Error>;

/// Removes one name only if it still selects the captured key.
/// Example: unlink an alias without closing existing access. Nonempty
/// containers reject. Accepted work reports Unit and at most WRITE activity.
pub type RemoveFn<State> =
    fn(&mut State, &[u8], &[u8]) -> Result<Pending, Error>;

/// Links a retained source key into an absent name in the same namespace.
///
/// Example: add an alias without copying the subject. Runtime retains source
/// access; the provider revalidates the key and destination absence.
pub type LinkFn<State> = fn(&mut State, &[u8], &[u8]) -> Result<Pending, Error>;

/// Captured rename within one mounted namespace, e.g. moving a device alias.
///
/// All spans live through begin; pending providers capture retained inputs.
#[derive(Clone, Copy)]
pub struct Rename<'a> {
    /// Existing child name in the bound source parent.
    pub name: &'a [u8],
    /// Lookup key to revalidate before moving the name.
    pub key: &'a [u8],
    /// None selects the mounted root; Some selects a retained container key.
    pub destination: Option<&'a [u8]>,
    /// New exclusive child name in that destination.
    pub new_name: &'a [u8],
}
/// Atomic no-replace move, e.g. a rename preserving the source object key.
pub type RenameFn<State> = fn(&mut State, Rename<'_>) -> Result<Pending, Error>;

/// Default child factory: stage a new key/provider before publishing its name.
/// Example: an empty ordinary data object, with Unit output and WRITE activity.
pub type CreateChildFn<State> =
    fn(&mut State, &[u8], &Creation) -> Result<Pending, Error>;

/// Namespace callbacks paired with private provider state.
///
/// Returned names/keys are owned values; pending work captures borrowed inputs.
/// Runtime owns identity interning, caller authority and installation.
pub struct NamespaceOps<State> {
    /// Observes CONTAINER (1) or CONTAINER|EMPTY (3) as canonical U32.
    /// Example: an empty directory returns 3, with no references or activity.
    pub flags: fn(&mut State) -> Result<Pending, Error>,
    /// Optional child-name lookup; aliases may return the same live object key.
    pub lookup: Option<LookupFn<State>>,
    /// Optional default child creation; uses `Creation::keyed_resource`.
    pub create: Option<CreateChildFn<State>>,
    /// Optional enumeration; the byte limit bounds the complete returned page.
    pub list: Option<ListFn<State>>,
    /// Optional lazy subject staging under an installation-owned capability.
    pub materialize: Option<MaterializeFn<State>>,
    /// Optional name removal; never recursive or an object close.
    pub remove: Option<RemoveFn<State>>,
    /// Optional exclusive alias creation; Unit with at most WRITE activity.
    pub link: Option<LinkFn<State>>,
    /// Optional atomic move; Unit with at most WRITE activity on both parents.
    pub rename: Option<RenameFn<State>>,
}
impl<State> NamespaceOps<State> {
    pub(super) const fn info(&self) -> NamespaceInfo {
        NamespaceInfo {
            lookup: self.lookup.is_some(),
            create: self.create.is_some(),
            list: self.list.is_some(),
            materialize: self.materialize.is_some(),
            remove: self.remove.is_some(),
            link: self.link.is_some(),
            rename: self.rename.is_some(),
        }
    }
}

pub(super) fn name(name: &[u8]) -> Result<(), Error> {
    if name.is_empty()
        || name == b"."
        || name == b".."
        || name.contains(&0)
        || name.contains(&b'/')
    {
        return Err(Error::INVALID);
    }
    if name.len() > 4096 {
        return Err(Error::PRESSURE);
    }
    // The owning namespace applies ASCII/opt-in policy before invocation.
    Ok(())
}
