use alloc::rc::Rc;
use alloc::vec::Vec;
use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::interaction::descriptor::{
    InteractionSessionDescriptor, RepresentationAttachmentDescriptor,
};
use reovim_kernel_interface::interaction::notification::{
    BufferChangeNotification, BufferChangeSubscription,
};
use reovim_kernel_interface::interaction::root::RootClientHandle;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateIdentityStamp,
};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use crate::{RuntimeError, capacity, reserved};

mod admission;
mod changes;
mod delivery;
mod history;
mod port;
mod record;
mod teardown;

pub use record::{RequestBody, RequestRecord};

/// Assembly-selected live authority and one supported Representation stream.
pub struct ClientBinding {
    /// Exact pre-issued Root authorization for this client and resource scope.
    pub authorization: LifecycleIdentity,
    /// Immutable Session contract; retains its own independent reservations.
    pub session: InteractionSessionDescriptor,
    /// Selected semantic attachment; no concrete renderer or cursor state.
    pub attachment: RepresentationAttachmentDescriptor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Inert,
    Opened,
    Negotiated,
    Attached,
    Closing,
    Closed,
}

/// Public endpoint containing delivery ownership, never authoritative state.
pub struct Session {
    pub(crate) binding: ClientBinding,
    pub(crate) plans: BufferPlanDescriptor,
    pub(crate) client: Option<RootClientHandle>,
    pub(crate) phase: Phase,
    pub(crate) current: Option<BufferStateStamp>,
    pub(crate) requests: Vec<RequestRecord>,
    pub(crate) request_capacity: usize,
    pub(crate) request_bytes: u64,
    pub(crate) projection: ProjectionStamp,
    pub(crate) transferred_projection: ProjectionStamp,
    pub(crate) next_delivery_cursor: Option<u64>,
    pub(crate) cursor: u64,
    pub(crate) initial: Option<RepresentationUpdate>,
    pub(crate) updates: Vec<Option<RepresentationUpdate>>,
    pub(crate) issued: Vec<RepresentationUpdateIdentityStamp>,
    pub(crate) acknowledged: Option<RepresentationUpdateIdentityStamp>,
    pub(crate) control: Option<RepresentationUpdate>,
    pub(crate) invalidated: bool,
    pub(crate) subscription: Option<BufferChangeSubscription>,
    pub(crate) subscription_closed: bool,
    pub(crate) notices: Vec<BufferChangeNotification>,
    pub(crate) notice_gap: Option<BufferStateStamp>,
    pub(crate) notice_frontier: Option<BufferStateStamp>,
    pub(crate) telemetry: Rc<crate::diagnostics::Telemetry>,
}

impl Session {
    pub(crate) fn new(
        binding: ClientBinding,
        plans: BufferPlanDescriptor,
        telemetry: Rc<crate::diagnostics::Telemetry>,
    ) -> Result<Self, RuntimeError> {
        if !binding.attachment.scope().same_scope(binding.session.scope())
            || !binding
                .attachment
                .handle()
                .session()
                .matches(binding.session.stamp())
        {
            return Err(RuntimeError::Configuration);
        }
        let requests = binding.session.bounds().requests();
        let request_capacity =
            capacity(requests.retained_outcomes().aggregate().value())?;
        let updates = binding.session.bounds().updates();
        let update_capacity =
            capacity(updates.retained_updates().aggregate().value())?;
        let credit = capacity(binding.attachment.initial_credit().value())?;
        if request_capacity == 0 || update_capacity == 0 || credit == 0 {
            return Err(RuntimeError::Configuration);
        }
        Ok(Self {
            projection: binding.attachment.projection(),
            transferred_projection: binding.attachment.projection(),
            next_delivery_cursor: Some(1),
            binding,
            plans,
            client: None,
            phase: Phase::Inert,
            current: None,
            requests: reserved(request_capacity)?,
            request_capacity,
            request_bytes: 0,
            cursor: 0,
            initial: None,
            updates: reserved(update_capacity)?,
            issued: reserved(credit)?,
            acknowledged: None,
            control: None,
            invalidated: false,
            subscription: None,
            subscription_closed: true,
            notices: Vec::new(),
            notice_gap: None,
            notice_frontier: None,
            telemetry,
        })
    }

    pub(crate) const fn ready(&self) -> bool {
        matches!(
            self.phase,
            Phase::Opened | Phase::Negotiated | Phase::Attached
        )
    }

    pub(crate) const fn ending(&self) -> bool {
        matches!(self.phase, Phase::Closing | Phase::Closed)
    }
}
