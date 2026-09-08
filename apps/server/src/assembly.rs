//! Explicitly selected local Resource, semantic providers and Host grants.

use alloc::{rc::Rc, vec::Vec};
use core::fmt;

use reovim_domain_basic::{HeadingDomain, KeyedDomain};
use reovim_kernel_core::{BufferConfig, BufferLimits};
use reovim_kernel_interface::buffer::{BufferIdentity, BufferIncarnationStamp};
use reovim_kernel_interface::interaction::root::{RootLimits, RootOpenTarget};
use reovim_kernel_interface::interaction::session::InteractionResourceScope;
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::state::{
    ResourceStateLimits, ResourceStateModel,
};
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity,
};
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_kernel_runtime::{
    Providers, ResourceRequests, Runtime, RuntimeConfig,
};
use reovim_representation_basic::{
    HeadingRepresentation, HeadingSnapshot, KeyedRepresentation, KeyedSnapshot,
};
use reovim_resource_local::{ByteModel, KeyedModel, MemoryResource, profile};

use crate::client::Client;

mod bindings;
pub use bindings::client_binding;

/// Initial local-reference capacity; not a product performance budget.
pub const HISTORY_NODES: u64 = 256;

/// Assembly errors do not publish a partial service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssemblyError {
    /// Bounded identity namespace cannot represent a child owner.
    Identity,
    /// Resource bytes or schema are unsupported by the selected profile.
    Resource,
    /// Selected semantic provider binding is incompatible.
    Policy,
    /// Runtime selections or reservations could not be constructed.
    Runtime,
}
impl fmt::Display for AssemblyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "local assembly: {self:?}")
    }
}
impl core::error::Error for AssemblyError {}

/// Concrete heading composition; no provider implementation enters Kernel.
pub type HeadingService =
    Service<ByteModel, HeadingDomain, HeadingRepresentation, HeadingSnapshot>;
/// The same Runtime/Core algorithms with a non-byte native family.
pub type KeyedService =
    Service<KeyedModel, KeyedDomain, KeyedRepresentation, KeyedSnapshot>;

/// Assembly and two public consumers, with no hidden state mutation hooks.
pub struct Service<M: ResourceStateModel, D, R, S> {
    /// Actual Kernel Runtime to drive through bounded progress.
    pub runtime: Runtime<MemoryResource, M, D, R, S>,
    /// Independent public request/snapshot consumers.
    pub clients: Vec<Client>,
    /// The selected resource operation; a client explicitly requests open.
    pub open_target: RootOpenTarget,
    /// Shared diagnostic observation, not the semantic update feed.
    pub telemetry: Rc<Telemetry>,
}

/// Assembles the bounded heading profile over real immutable source bytes.
///
/// # Errors
/// Returns identity, Resource, policy or Runtime selection failure.
pub fn heading(
    owner: LifecycleIdentity,
    source: Vec<u8>,
    telemetry: Rc<Telemetry>,
) -> Result<HeadingService, AssemblyError> {
    let model = ByteModel::new(child(owner, 1)?, child(owner, 2)?);
    let domain = HeadingDomain::new(&model);
    let representation = HeadingRepresentation::new(&domain, &model)
        .map_err(|_| AssemblyError::Policy)?;
    let snapshot = HeadingSnapshot::new(&representation);
    assemble(
        owner,
        source,
        Providers { resource: (), model, domain, representation, snapshot },
        telemetry,
    )
}

/// Assembles keyed toggles through the same Kernel algorithms.
///
/// # Errors
/// Returns identity, Resource, policy or Runtime selection failure.
pub fn keyed(
    owner: LifecycleIdentity,
    source: Vec<u8>,
    telemetry: Rc<Telemetry>,
) -> Result<KeyedService, AssemblyError> {
    let model = KeyedModel::new(child(owner, 1)?, child(owner, 2)?);
    let domain = KeyedDomain::new(&model);
    let representation = KeyedRepresentation::new(&domain, &model)
        .map_err(|_| AssemblyError::Policy)?;
    let snapshot = KeyedSnapshot::new(&representation);
    assemble(
        owner,
        source,
        Providers { resource: (), model, domain, representation, snapshot },
        telemetry,
    )
}

fn assemble<M, D, R, S>(
    owner: LifecycleIdentity,
    source: Vec<u8>,
    providers: Providers<(), M, D, R, S>,
    telemetry: Rc<Telemetry>,
) -> Result<Service<M, D, R, S>, AssemblyError>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    // Reserve a nonoverlapping component namespace before constructing owners.
    child(owner, 999)?;
    let Providers { model, domain, representation, snapshot, .. } = providers;
    let access_life = child(owner, 3)?;
    let access = ResourceAccessStamp::new(
        ResourceIdentity::new(access_life.instance()),
        access_life,
    );
    let resource = MemoryResource::new(&model.binding(), access, source)
        .map_err(|_| AssemblyError::Resource)?;
    let open_target = RootOpenTarget::Independent(resource.open_request(1));
    let requests = ResourceRequests {
        observation: resource.observation_request(2),
        close: resource.close_request(),
    };
    let buffer_life = child(owner, 4)?;
    let incarnation = BufferIncarnationStamp::new(
        BufferIdentity::new(buffer_life.instance()),
        buffer_life,
    );
    let config = RuntimeConfig {
        owner,
        limits: RootLimits {
            clients: profile::count(2),
            opens: profile::count(4),
            work: profile::count(64),
        },
        buffer: BufferConfig {
            source: resource.snapshot(),
            incarnation,
            history: child(owner, 5)?,
            model: model.binding(),
            plan_owner: model.plan_descriptor().owner(),
            limits: BufferLimits {
                history_nodes: profile::count(HISTORY_NODES),
                resident_bytes: profile::bytes(32 * 1024 * 1024),
            },
            preparation: ResourceStateLimits {
                resident_bytes: profile::bytes(
                    reovim_resource_local::MAX_SOURCE_BYTES + 1024,
                ),
                work_units: profile::count(
                    reovim_resource_local::MAX_SOURCE_BYTES + 1024,
                ),
                observation_count: profile::count(1),
                observation_bytes: profile::bytes(
                    reovim_resource_local::MAX_SOURCE_BYTES,
                ),
            },
        },
    };
    let scope = InteractionResourceScope::new(access, incarnation);
    let mut bindings = Vec::new();
    let mut clients = Vec::new();
    for role in [100, 200] {
        let binding = bindings::client_binding(
            child(owner, role)?,
            scope,
            representation.descriptor(),
            snapshot.descriptor(),
        )?;
        clients.push(Client::new(
            &binding,
            representation.descriptor().clone(),
            model.plan_descriptor().clone(),
        ));
        bindings.push(binding);
    }
    let runtime = Runtime::new(
        config,
        Providers { resource, model, domain, representation, snapshot },
        requests,
        bindings,
        Rc::clone(&telemetry),
    )
    .map_err(|_| AssemblyError::Runtime)?;
    Ok(Service { runtime, clients, open_target, telemetry })
}

/// Constructs an explicit reference root incarnation for a caller-owned run.
#[must_use]
pub const fn life(instance: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(instance, instance, Generation::new(1))
}

fn child(
    owner: LifecycleIdentity,
    role: u128,
) -> Result<LifecycleIdentity, AssemblyError> {
    let instance = owner
        .instance()
        .checked_mul(1000)
        .and_then(|base| base.checked_add(role))
        .ok_or(AssemblyError::Identity)?;
    Ok(LifecycleIdentity::new(
        instance,
        owner.incarnation(),
        owner.generation(),
    ))
}
