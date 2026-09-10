use reovim_domain_basic::HeadingDomain;
use reovim_host_reference::DeterministicClock;
use reovim_kernel_core::{BufferConfig, BufferLimits};
use reovim_kernel_interface::buffer::{BufferIdentity, BufferIncarnationStamp};
use reovim_kernel_interface::cancellation::DeadlineState;
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::root::{self, InteractionRootPort};
use reovim_kernel_interface::interaction::session::InteractionResourceScope;
use reovim_kernel_interface::resource::state::{
    ResourceStateLimits, ResourceStateModel,
};
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity,
};
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_kernel_runtime::{
    Providers, ResourceRequests, Runtime, RuntimeConfig,
};
use reovim_representation_basic::{HeadingRepresentation, HeadingSnapshot};
use reovim_resource_local::{ByteModel, MemoryResource, profile};
use reovim_server::{
    assembly::{self, life},
    client::Client,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use super::providers::{
    Control, Fault, FaultModel, FaultResource, FaultSnapshot,
};

pub struct Harness {
    pub runtime: Runtime<
        FaultResource,
        FaultModel,
        HeadingDomain,
        HeadingRepresentation,
        FaultSnapshot,
    >,
    pub clients: Vec<Client>,
    pub handles: Vec<root::RootClientHandle>,
    pub control: Rc<Control>,
    pub target: root::RootOpenTarget,
}
impl Harness {
    pub fn budget(&self) -> root::RootLifecycleRequest {
        root::RootLifecycleRequest {
            root: self.runtime.owner(),
            work: profile::count(64),
            deadline: DeadlineState::Pending,
        }
    }
    pub fn open(
        &mut self,
        client: usize,
        target: root::RootOpenTarget,
        id: u128,
    ) -> root::RootOpenTicket {
        self.runtime
            .open(root::RootOpenRequest {
                client: self.handles[client],
                request: InteractionRequestIdentity::new(id),
                target,
                work: profile::count(64),
            })
            .unwrap()
    }
    pub fn assert_unpublished(&mut self, ticket: root::RootOpenTicket) {
        let buffer = self.clients[0].session.scope().buffer();
        assert!(self.runtime.endpoint(ticket.client, buffer).is_err());
        assert_eq!(
            self.runtime.resolve(self.handles[1], "buffer"),
            Err(root::RootFailure::Unavailable)
        );
    }
}

pub fn fixture(fault: Fault) -> Harness {
    let control = Rc::new(Control {
        fault,
        release: Cell::new(false),
        native_fail: Cell::new(false),
        snapshot_fail: Cell::new(false),
        replaced: Cell::new(false),
        operation: Cell::new(None),
        calls: RefCell::new(Vec::new()),
    });
    let model = ByteModel::new(life(2), life(3));
    let domain = HeadingDomain::new(&model);
    let representation = HeadingRepresentation::new(&domain, &model).unwrap();
    let snapshot = HeadingSnapshot::new(&representation);
    let access = ResourceAccessStamp::new(ResourceIdentity::new(4), life(4));
    let resource =
        MemoryResource::new(&model.binding(), access, b"# a\n\xff".to_vec())
            .unwrap();
    let requests = ResourceRequests {
        open: resource.open_request(1),
        observation: resource.observation_request(2),
        close: resource.close_request(),
    };
    let config = configuration(&resource, &model);
    let buffer = config.buffer.incarnation;
    let bindings = [100, 200].map(|id| {
        assembly::client_binding(
            life(id),
            InteractionResourceScope::new(access, buffer),
            representation.descriptor(),
            snapshot.descriptor(),
        )
        .unwrap()
    });
    let clients = bindings
        .iter()
        .map(|binding| {
            Client::new(
                binding.session.clone(),
                binding.attachment.clone(),
                binding.authorization,
                representation.descriptor().clone(),
                model.plan_descriptor().clone(),
            )
        })
        .collect::<Vec<_>>();
    let mut runtime = Runtime::new(
        config,
        Providers {
            resource: FaultResource::new(resource, control.clone()),
            model: FaultModel { inner: model, control: control.clone() },
            domain,
            representation,
            snapshot: FaultSnapshot {
                inner: snapshot,
                control: control.clone(),
            },
        },
        requests,
        bindings.into(),
        telemetry(),
    )
    .unwrap();
    runtime
        .start(root::RootLifecycleRequest {
            root: life(1),
            work: profile::count(64),
            deadline: DeadlineState::Pending,
        })
        .unwrap();
    let handles = clients
        .iter()
        .map(|client| {
            runtime.attach_client(client.attach_request(life(1))).unwrap()
        })
        .collect::<Vec<_>>();
    let target = root::RootOpenTarget::Independent(
        runtime.resolve(handles[0], "source").unwrap(),
    );
    Harness { runtime, clients, handles, control, target }
}

fn configuration(
    resource: &MemoryResource,
    model: &ByteModel,
) -> RuntimeConfig {
    RuntimeConfig {
        owner: life(1),
        limits: root::RootLimits {
            clients: profile::count(2),
            opens: profile::count(4),
            work: profile::count(64),
        },
        buffer: BufferConfig {
            source: resource.snapshot(),
            incarnation: BufferIncarnationStamp::new(
                BufferIdentity::new(5),
                life(5),
            ),
            history: life(6),
            model: model.binding(),
            plan_owner: model.plan_descriptor().owner(),
            limits: BufferLimits {
                history_nodes: profile::count(16),
                resident_bytes: profile::bytes(8192),
            },
            preparation: ResourceStateLimits {
                resident_bytes: profile::bytes(1024),
                work_units: profile::count(64),
                observation_count: profile::count(1),
                observation_bytes: profile::bytes(1024),
            },
        },
    }
}

fn telemetry() -> Rc<Telemetry> {
    let descriptor = ClockDescriptor::new(
        profile::schema(90),
        life(7),
        life(1),
        1,
        u64::MAX,
        91,
    )
    .unwrap();
    Rc::new(
        Telemetry::new(
            life(1),
            life(8),
            Rc::new(DeterministicClock::new(descriptor)),
            None,
            32,
        )
        .unwrap(),
    )
}
