use std::{cell::Cell, rc::Rc};

use reovim_domain_basic::HeadingDomain;
use reovim_host_reference::DeterministicClock;
use reovim_kernel_core::{BufferConfig, BufferLimits};
use reovim_kernel_interface::buffer::{BufferIdentity, BufferIncarnationStamp};
use reovim_kernel_interface::cancellation::DeadlineState;
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::interaction::delivery::{intent, update};
use reovim_kernel_interface::interaction::identity::{
    InteractionRequestIdentity, RepresentationUpdateCursor,
};
use reovim_kernel_interface::interaction::outcome::InteractionOutcome;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root;
use reovim_kernel_interface::interaction::root::InteractionRootPort;
use reovim_kernel_interface::interaction::session::InteractionResourceScope;
use reovim_kernel_interface::interaction::update::RepresentationUpdate;
use reovim_kernel_interface::resource::port::ResourceAccessPort;
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
    Providers, ResourceRequests, Runtime, RuntimeConfig, Session,
};
use reovim_representation_basic::{HeadingRepresentation, HeadingSnapshot};
use reovim_resource_local::{ByteModel, MemoryResource, profile};
use reovim_server::{assembly, client::Client};

use super::providers::{FaultSnapshot, SnapshotMode};

pub struct Harness<A> {
    pub runtime: Runtime<
        A,
        ByteModel,
        HeadingDomain,
        HeadingRepresentation,
        FaultSnapshot,
    >,
    pub client: Client,
    pub target: root::RootOpenTarget,
    pub telemetry: Rc<Telemetry>,
}

pub fn build<A>(
    wrap: impl FnOnce(MemoryResource) -> A,
    mode: Rc<Cell<SnapshotMode>>,
    pressure: bool,
) -> Harness<A>
where
    A: ResourceAccessPort,
{
    let model = ByteModel::new(assembly::life(2), assembly::life(3));
    let domain = HeadingDomain::new(&model);
    let representation = HeadingRepresentation::new(&domain, &model).unwrap();
    let snapshot = HeadingSnapshot::new(&representation);
    let access =
        ResourceAccessStamp::new(ResourceIdentity::new(4), assembly::life(4));
    let resource =
        MemoryResource::new(&model.binding(), access, b"# a\n\xff".to_vec())
            .unwrap();
    let target = root::RootOpenTarget::Independent(resource.open_request(1));
    let requests = ResourceRequests {
        observation: resource.observation_request(2),
        close: resource.close_request(),
    };
    let buffer =
        BufferIncarnationStamp::new(BufferIdentity::new(5), assembly::life(5));
    let scope = InteractionResourceScope::new(access, buffer);
    let mut binding = assembly::client_binding(
        assembly::life(100),
        scope,
        representation.descriptor(),
        snapshot.descriptor(),
    )
    .expect("selected client");
    if pressure {
        super::retention::restrict(&mut binding);
    }
    let client = Client::new(
        &binding,
        representation.descriptor().clone(),
        model.plan_descriptor().owner(),
    );
    let config = RuntimeConfig {
        owner: assembly::life(1),
        limits: root::RootLimits {
            clients: profile::count(1),
            opens: profile::count(4),
            work: profile::count(64),
        },
        buffer: BufferConfig {
            source: resource.snapshot(),
            incarnation: buffer,
            history: assembly::life(6),
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
    };
    let snapshot = FaultSnapshot::new(snapshot, &representation, mode);
    let telemetry = telemetry();
    let runtime = Runtime::new(
        config,
        Providers {
            resource: wrap(resource),
            model,
            domain,
            representation,
            snapshot,
        },
        requests,
        vec![binding],
        Rc::clone(&telemetry),
    )
    .unwrap();
    Harness { runtime, client, target, telemetry }
}

pub fn telemetry() -> Rc<Telemetry> {
    let descriptor = ClockDescriptor::new(
        profile::schema(90),
        assembly::life(7),
        assembly::life(1),
        1,
        u64::MAX,
        91,
    )
    .unwrap();
    let clock = Rc::new(DeterministicClock::new(descriptor));
    Rc::new(
        Telemetry::new(assembly::life(1), assembly::life(8), clock, None, 32)
            .unwrap(),
    )
}

impl<A: ResourceAccessPort> Harness<A> {
    pub fn lifecycle(&self) -> root::RootLifecycleRequest {
        root::RootLifecycleRequest {
            root: self.runtime.owner(),
            work: profile::count(64),
            deadline: DeadlineState::Pending,
        }
    }

    pub fn begin(&mut self) -> root::RootOpenTicket {
        self.runtime.start(self.lifecycle()).unwrap();
        let handle = self
            .runtime
            .attach_client(self.client.attach_request(self.runtime.owner()))
            .unwrap();
        self.runtime
            .open(root::RootOpenRequest {
                client: handle,
                request: InteractionRequestIdentity::new(1),
                target: self.target,
                work: profile::count(64),
            })
            .unwrap()
    }

    pub fn boot(&mut self) {
        let ticket = self.begin();
        self.runtime.progress(profile::count(64)).unwrap();
        let ready =
            self.runtime.take_open(ticket).unwrap().unwrap().outcome.unwrap();
        self.client.opened(ticket.client, ready).unwrap();
        let port =
            self.runtime.endpoint(ticket.client, ready.state.buffer()).unwrap();
        self.client.initialize(port).unwrap();
    }

    pub fn port(&mut self) -> &mut Session {
        self.runtime
            .endpoint(
                self.client.handle().unwrap(),
                self.client.state().unwrap().buffer(),
            )
            .unwrap()
    }

    pub fn edit(&mut self, id: u128, value: &[u8]) -> InteractionOutcome {
        let request = self.client.intent(id, value).unwrap();
        let lookup = intent::InteractionOutcomeRequest::new(
            &self.client.session,
            request.stamp(),
        )
        .unwrap();
        let admitted = self.port().submit_intent(request);
        assert!(admitted.admitted_request().is_some());
        self.runtime.progress(profile::count(64)).unwrap();
        self.port().take_outcome(lookup).outcome().unwrap()
    }

    pub fn next_update(&mut self) -> RepresentationUpdate {
        let request = update::RepresentationUpdateRequest::new(
            &self.client.session,
            &self.client.attachment,
            RepresentationUpdateCursor::new(self.client.cursor() + 1),
        )
        .unwrap();
        self.port()
            .take_update(request)
            .into_update()
            .expect("ordered control or snapshot")
    }
}
