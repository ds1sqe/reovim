//! Public-only client observation, local selection and bounded delivery state.

use alloc::boxed::Box;
use core::fmt;

use reovim_kernel_interface::buffer::{
    BufferStateStamp, plan::BufferPlanDescriptor,
};
use reovim_kernel_interface::interaction::descriptor::{
    InteractionSessionDescriptor, RepresentationAttachmentDescriptor,
};
use reovim_kernel_interface::interaction::{
    notification::BufferChangeSubscription, root::RootClientHandle,
};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_interface::semantic::descriptor;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

mod lifetime;
mod observation;
mod receive;
mod recovery;
mod request;

/// Concrete local selections and correspondence for the consumed profiles.
pub mod selection;

pub use receive::EditExpectation;
pub use selection::{
    KeySet, PartialAcceptance, QualifiedSelection, ResponsePolicy,
    SelectionError, SelectionStatus, SelectionTranslation, TranslationStatus,
};

/// Expected local failure; never a reversal of Kernel acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// Initial negotiation or attachment has not supplied usable state.
    NotReady,
    /// A received value disagrees with the selected public binding.
    Binding,
    /// State, cursor or predecessor is no longer usable.
    Stale,
    /// Lost continuity requires a validated recovery snapshot.
    Invalidated,
    /// Semantic instruction does not fit its selected schema/bounds.
    Intent,
    /// The local request/cursor identity cannot advance.
    Identity,
    /// A bounded rendezvous slot is already occupied.
    Pressure,
    /// Actual acceptance and snapshot evidence disagree.
    Mismatch,
    /// Local selection interpretation failed; observed state is independent.
    Selection(SelectionError),
    /// Recovery completed without a usable semantic snapshot.
    Recovery(reovim_kernel_interface::interaction::delivery::update::
        RepresentationResynchronizationFailure),
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "reference client: {self:?}")
    }
}
impl core::error::Error for ClientError {}

/// Independent observation and local policy, with no native-state authority.
pub struct Client {
    /// Selected public Session descriptor.
    pub session: InteractionSessionDescriptor,
    /// Currently observed Representation binding, never Kernel storage.
    pub attachment: RepresentationAttachmentDescriptor,
    policy: descriptor::RepresentationPolicyDescriptor,
    authorization: LifecycleIdentity,
    plans: BufferPlanDescriptor,
    handle: Option<RootClientHandle>,
    state: Option<BufferStateStamp>,
    projection: ProjectionStamp,
    payload: Option<Box<[u8]>>,
    cursor: u64,
    selection: Option<QualifiedSelection>,
    response: ResponsePolicy,
    selection_issue: Option<SelectionError>,
    subscription: Option<BufferChangeSubscription>,
    subscription_frontier: Option<BufferStateStamp>,
    pending: receive::Pending,
    recovery: recovery::Recovery,
}

impl Client {
    /// Constructs an inert consumer from assembly-selected public contracts.
    #[must_use]
    pub fn new(
        binding: &reovim_kernel_runtime::ClientBinding,
        policy: descriptor::RepresentationPolicyDescriptor,
        plans: BufferPlanDescriptor,
    ) -> Self {
        Self {
            session: binding.session.clone(),
            attachment: binding.attachment.clone(),
            projection: binding.attachment.projection(),
            policy,
            authorization: binding.authorization,
            plans,
            handle: None,
            state: None,
            payload: None,
            cursor: 0,
            selection: None,
            response: ResponsePolicy::Keep,
            selection_issue: None,
            subscription: None,
            subscription_frontier: None,
            pending: receive::Pending::default(),
            recovery: recovery::Recovery::default(),
        }
    }

    /// Returns the latest exact observation used by subsequent requests.
    #[must_use]
    pub const fn state(&self) -> Option<BufferStateStamp> {
        self.state
    }

    /// Returns current interpreted bytes, never authoritative native storage.
    #[must_use]
    pub fn payload(&self) -> Option<&[u8]> {
        self.payload.as_deref()
    }

    /// Returns the last consumed stream position for ordered retrieval.
    #[must_use]
    pub const fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Returns this client's live root attachment once opened.
    #[must_use]
    pub const fn handle(&self) -> Option<RootClientHandle> {
        self.handle
    }
}
