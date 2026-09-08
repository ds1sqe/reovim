use std::rc::Rc;

use reovim_host_reference::DeterministicClock;
use reovim_kernel_interface::cancellation::DeadlineState;
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::event::editing::EditingStage;
use reovim_kernel_interface::event::trace::{
    SemanticCaptureKey, TraceLimits, TraceNode, TraceQueryBudget,
    TraceReadPort, TraceReport, TraceStart,
};
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeStatus;
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;
use reovim_server::assembly::{self, HeadingService, life};

pub const fn grant() -> LifecycleIdentity {
    life(90)
}

pub fn limits() -> TraceLimits {
    TraceLimits {
        nodes: profile::count(64),
        snapshots: profile::count(16),
        metadata_bytes: profile::bytes(1024 * 1024),
        payload_bytes: profile::bytes(8192),
    }
}

pub fn budget() -> TraceQueryBudget {
    TraceQueryBudget {
        primary: true,
        joins: true,
        nodes: profile::count(64),
        edges: profile::count(128),
        depth: profile::count(16),
        work: profile::count(10_000),
        bytes: profile::bytes(1024 * 1024),
        deadline: DeadlineState::Pending,
    }
}

pub fn telemetry(selected: Option<TraceLimits>) -> Rc<Telemetry> {
    let clock = ClockDescriptor::new(
        profile::schema(91),
        life(92),
        life(1),
        1,
        u64::MAX,
        93,
    )
    .unwrap();
    let telemetry = Telemetry::new(
        life(1),
        life(94),
        Rc::new(DeterministicClock::new(clock)),
        None,
        16,
    )
    .unwrap();
    let Some(limits) = selected else {
        return Rc::new(telemetry);
    };
    // The assembly-owned read grant is not given to ordinary clients.
    Rc::new(telemetry.with_tracing(grant(), limits).unwrap())
}

pub fn service(telemetry: Rc<Telemetry>) -> HeadingService {
    let mut service =
        assembly::heading(life(1), b"# a\n\xff".to_vec(), telemetry).unwrap();
    service.open(0).unwrap();
    service.open(1).unwrap();
    service
}

pub fn edit(service: &mut HeadingService, request: u128, label: &[u8]) {
    let outcome = service.edit(0, request, label).unwrap();
    assert_eq!(outcome.status(), InteractionOutcomeStatus::Accepted);
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].state(), service.clients[1].state());
    assert_eq!(service.clients[0].payload(), service.clients[1].payload());
}

pub fn request(service: &HeadingService, id: u128) -> TraceStart {
    TraceStart::Request {
        session: service.clients[0].session.stamp().handle(),
        request: InteractionRequestIdentity::new(id),
    }
}

pub fn report(service: &HeadingService, id: u128) -> TraceReport {
    service
        .telemetry
        .backtrace(grant(), request(service, id), budget())
        .unwrap()
}

pub fn stage(report: &TraceReport, stage: EditingStage) -> TraceNode {
    *report.nodes.iter().find(|node| node.stage == stage).unwrap()
}

pub const fn key(node: &TraceNode) -> SemanticCaptureKey {
    SemanticCaptureKey {
        update: node.context.update.unwrap(),
        state: node.context.state.unwrap(),
    }
}

pub fn stages(report: &TraceReport) -> Vec<EditingStage> {
    let mut nodes = report.nodes.clone();
    nodes.sort_by_key(|node| node.location.sequence.value());
    nodes.into_iter().map(|node| node.stage).collect()
}
