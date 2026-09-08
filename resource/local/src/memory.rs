use alloc::vec::Vec;
use core::fmt;

use reovim_kernel_interface::resource::ResourceSnapshotStamp;
use reovim_kernel_interface::resource::access::ResourceAccessDescriptor;
use reovim_kernel_interface::resource::delivery::{
    ResourceObservationCompletion, ResourceOpenCompletion,
};
use reovim_kernel_interface::resource::operation::ResourceOperationStamp;
use reovim_kernel_interface::resource::state::ResourceStateModelBinding;
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceRevision, ResourceSnapshotIdentity,
};

mod descriptor;
mod port;
mod requests;

/// Construction failure of the bounded immutable source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryResourceError {
    /// Source allocation exceeds the reference profile's retained capacity.
    Pressure,
    /// This adapter does not understand the selected native payload schema.
    UnsupportedModel,
}

impl fmt::Display for MemoryResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pressure => "local source capacity exceeded",
            Self::UnsupportedModel => "unsupported local source model",
        })
    }
}

impl core::error::Error for MemoryResourceError {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Fresh,
    Ready,
    Closing,
    Closed,
}

/// One immutable source with bounded, exact-stamp deferred terminal delivery.
///
/// The reference Access admits one open and one initial observation. Completed
/// request identities stay fenced until close. This is not a writable file
/// adapter; subsequent edits belong to a Core-owned Buffer branch.
pub struct MemoryResource {
    descriptor: ResourceAccessDescriptor,
    snapshot: ResourceSnapshotStamp,
    source: Vec<u8>,
    state: State,
    open: Option<ResourceOpenCompletion>,
    observation: Option<ResourceObservationCompletion>,
    open_stamp: Option<ResourceOperationStamp>,
    observation_stamp: Option<ResourceOperationStamp>,
}

impl MemoryResource {
    /// Owns bounded source data under an assembly-selected Access identity.
    ///
    /// # Errors
    /// Rejects unsupported native payloads or source allocation pressure.
    ///
    /// # Panics
    /// Only if fixed reference-profile declarations become incoherent.
    pub fn new(
        model: &ResourceStateModelBinding,
        access: ResourceAccessStamp,
        source: Vec<u8>,
    ) -> Result<Self, MemoryResourceError> {
        if !matches!(model.contract.identity(), 0x1000 | 0x2000)
            || *model
                != crate::native::binding(
                    model.contract.identity(),
                    model.lifecycle,
                )
        {
            return Err(MemoryResourceError::UnsupportedModel);
        }
        let allocated = u64::try_from(source.capacity())
            .map_err(|_| MemoryResourceError::Pressure)?;
        if allocated > crate::MAX_SOURCE_BYTES {
            return Err(MemoryResourceError::Pressure);
        }
        let descriptor = descriptor::build(model, access);
        let snapshot = ResourceSnapshotStamp::new(
            access,
            ResourceRevision::new(model.schemas.revision(), 1),
            ResourceSnapshotIdentity::new(access.resource().value()),
        );
        Ok(Self {
            descriptor,
            snapshot,
            source,
            state: State::Fresh,
            open: None,
            observation: None,
            open_stamp: None,
            observation_stamp: None,
        })
    }

    /// Returns immutable source identity for assembly configuration.
    #[must_use]
    pub const fn snapshot(&self) -> ResourceSnapshotStamp {
        self.snapshot
    }
}
