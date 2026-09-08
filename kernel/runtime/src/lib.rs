//! Single-threaded bounded editing service; Core alone installs state.
#![no_std]

extern crate alloc;

pub mod diagnostics;
mod engine;
mod history;
mod opening;
mod recovery;
mod root;
mod session;
mod snapshot;

use alloc::rc::Rc;
use alloc::vec::Vec;
use reovim_kernel_core::{Buffer, BufferConfig};
use reovim_kernel_interface::interaction::root::{RootLimits, RootOpenRequest};
use reovim_kernel_interface::lifecycle::{LifecycleIdentity, LifecycleState};
use reovim_kernel_interface::resource::operation::{
    ResourceCloseRequest, ResourceObservationRequest,
};
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::descriptor::{
    DomainPolicyDescriptor, RepresentationPolicyDescriptor,
};
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::{
    RepresentationSnapshotDescriptor, RepresentationSnapshotPort,
};

pub use session::{ClientBinding, Session};

/// Explicit reservations and owner identities for one selected Resource.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeConfig {
    /// Root lifecycle selected by assembly.
    pub owner: LifecycleIdentity,
    /// Client, request ledger and per-tick work limits.
    pub limits: RootLimits,
    /// Core identity and native-state reservations.
    pub buffer: BufferConfig,
}

/// Construction/progress failure with no fabricated editing acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// Supplied selections or zero reservations are incompatible.
    Configuration,
    /// A reservation cannot be represented or allocated.
    Capacity,
    /// A provider violated the selected contract.
    Provider,
}

/// Assembly-owned selected implementations, with no global discovery.
pub struct Providers<A, M, D, R, S> {
    /// Selected Resource Access adapter.
    pub resource: A,
    /// Native immutable working-state model.
    pub model: M,
    /// Semantic validation policy.
    pub domain: D,
    /// Semantic-to-native lowering policy.
    pub representation: R,
    /// Native-state semantic snapshot producer.
    pub snapshot: S,
}

/// Native region and release requests selected outside neutral mechanisms.
pub struct ResourceRequests {
    /// Initial observation, retained until actual open readiness.
    pub observation: ResourceObservationRequest,
    /// Matching Resource lifetime close request.
    pub close: ResourceCloseRequest,
}

/// Selected providers; no registry or concrete domain enters the Kernel.
pub struct Runtime<A, M, D, R, S>
where
    M: ResourceStateModel,
{
    config: RuntimeConfig,
    phase: LifecycleState,
    resource: A,
    model: M,
    domain: D,
    representation: R,
    snapshot: S,
    domain_selection: DomainPolicyDescriptor,
    representation_selection: RepresentationPolicyDescriptor,
    snapshot_selection: RepresentationSnapshotDescriptor,
    observation: Option<ResourceObservationRequest>,
    resource_close: ResourceCloseRequest,
    buffer: Option<Buffer<M::State>>,
    sessions: Vec<Session>,
    opens: Vec<opening::OpenRecord>,
    opening: Option<opening::Opening>,
    telemetry: Rc<diagnostics::Telemetry>,
    next_session: usize,
}

impl<A, M, D, R, S> Runtime<A, M, D, R, S>
where
    A: ResourceAccessPort,
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    /// Reports Core-retained native preparation charges, not physical RSS.
    #[must_use]
    pub fn retained_native_bytes(
        &self,
    ) -> Option<reovim_kernel_interface::bound::CheckedByteCount> {
        self.buffer.as_ref().map(Buffer::resident_bytes)
    }

    /// Reserves inert ownership. Native region selection belongs to assembly.
    ///
    /// # Errors
    /// Returns incompatible selections or unavailable bounded storage.
    pub fn new(
        config: RuntimeConfig,
        providers: Providers<A, M, D, R, S>,
        requests: ResourceRequests,
        clients: Vec<ClientBinding>,
        telemetry: Rc<diagnostics::Telemetry>,
    ) -> Result<Self, RuntimeError> {
        let Providers { resource, model, domain, representation, snapshot } =
            providers;
        validate_config(&config, &model, &domain, &representation, &snapshot)?;
        if requests.observation.target().access()
            != config.buffer.source.access()
            || requests.observation.operation().adapter()
                != resource.descriptor().adapter()
            || requests.close.operation().adapter()
                != resource.descriptor().adapter()
        {
            return Err(RuntimeError::Configuration);
        }
        if clients.is_empty()
            || clients.len() > capacity(config.limits.clients.value())?
        {
            return Err(RuntimeError::Configuration);
        }
        let mut sessions = reserved(clients.len())?;
        for binding in clients {
            validate_client(&config, &binding, snapshot.descriptor())?;
            sessions.push(Session::new(
                binding,
                model.plan_descriptor().clone(),
                Rc::clone(&telemetry),
            )?);
        }
        Ok(Self {
            config,
            phase: LifecycleState::Constructed,
            resource,
            model,
            domain_selection: domain.descriptor().clone(),
            representation_selection: representation.descriptor().clone(),
            snapshot_selection: snapshot.descriptor().clone(),
            domain,
            representation,
            snapshot,
            observation: Some(requests.observation),
            resource_close: requests.close,
            buffer: None,
            sessions,
            opens: reserved(capacity(config.limits.opens.value())?)?,
            opening: None,
            telemetry,
            next_session: 0,
        })
    }

    fn open_index(&self, request: &RootOpenRequest) -> Option<usize> {
        self.opens.iter().position(|record| {
            record.request.client == request.client
                && record.request.request == request.request
        })
    }

    fn providers_current(&self) -> bool {
        self.model.binding() == self.config.buffer.model
            && *self.domain.descriptor() == self.domain_selection
            && *self.representation.descriptor()
                == self.representation_selection
    }
}

fn validate_client(
    config: &RuntimeConfig,
    binding: &ClientBinding,
    snapshot: &RepresentationSnapshotDescriptor,
) -> Result<(), RuntimeError> {
    use reovim_kernel_interface::interaction::descriptor;
    let unsupported = descriptor::InteractionCapabilitySupport::Unsupported;
    let capabilities = binding.session.capabilities();
    if binding.session.scope().buffer() != config.buffer.incarnation
        || binding.session.scope().access() != config.buffer.source.access()
        || binding.attachment.representation() != snapshot.representation()
        || binding.attachment.coordinate() != snapshot.coordinate()
        || binding.session.update_contract().schema() != snapshot.output()
        || capabilities.deltas() != unsupported
    {
        return Err(RuntimeError::Configuration);
    }
    Ok(())
}

fn validate_config<M, D, R, S>(
    config: &RuntimeConfig,
    model: &M,
    domain: &D,
    representation: &R,
    snapshot: &S,
) -> Result<(), RuntimeError>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    if config.limits.work.value() == 0 || config.limits.opens.value() == 0 {
        return Err(RuntimeError::Configuration);
    }
    let binding = model.binding();
    if config.buffer.model != binding
        || *domain.descriptor().state_model() != binding
        || *representation.descriptor().state_model() != binding
        || *snapshot.descriptor().state_model() != binding
        || snapshot.descriptor().representation()
            != representation.descriptor().stamp()
        || representation.descriptor().stamp().domain()
            != domain.descriptor().stamp()
    {
        return Err(RuntimeError::Configuration);
    }
    Ok(())
}

fn capacity(value: u64) -> Result<usize, RuntimeError> {
    usize::try_from(value).map_err(|_| RuntimeError::Capacity)
}

fn reserved<T>(capacity: usize) -> Result<Vec<T>, RuntimeError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| RuntimeError::Capacity)?;
    Ok(values)
}
