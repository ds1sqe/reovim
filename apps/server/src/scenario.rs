//! Actual public requests, shared by both hosted native-model scenarios.

use std::{error::Error, io};

use reovim_kernel_interface::buffer::BufferHistoryNodeIdentity;
use reovim_kernel_interface::interaction::history::InteractionHistoryAction;
use reovim_kernel_interface::interaction::outcome::{
    InteractionAcceptedState, InteractionOutcome,
};
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;
use reovim_server::assembly::Service;

pub fn exercise<M, D, R, S>(
    mut service: Service<M, D, R, S>,
    inputs: &[&[u8]],
    branch_input: &[u8],
) -> Result<(), Box<dyn Error>>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    service.open(0)?;
    service.open(1)?;
    for (index, input) in inputs.iter().enumerate() {
        let request = index as u128 + 1;
        accepted(&service.edit(0, request, input)?)?;
        observe(&mut service, request)?;
    }
    branch(&mut service, branch_input)?;
    service.close()?;
    Ok(())
}

fn branch<M, D, R, S>(
    service: &mut Service<M, D, R, S>,
    input: &[u8],
) -> Result<(), Box<dyn Error>>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    use InteractionHistoryAction::{Redo, Undo};
    let old_branch = service.clients[0]
        .state()
        .ok_or_else(|| io::Error::other("client has no observation"))?
        .history()
        .node();
    traverse(service, 50, Undo)?;
    let new_branch =
        accepted(&service.edit(0, 51, input)?)?.result().history().node();
    observe(service, 51)?;
    traverse(service, 52, Undo)?;
    if traverse(service, 53, Redo { child: None })? != new_branch {
        return Err(io::Error::other("default redo missed new child").into());
    }
    traverse(service, 54, Undo)?;
    if traverse(service, 55, Redo { child: Some(old_branch) })? != old_branch {
        return Err(
            io::Error::other("explicit redo missed retained sibling").into()
        );
    }
    Ok(())
}

fn traverse<M, D, R, S>(
    service: &mut Service<M, D, R, S>,
    request: u128,
    action: InteractionHistoryAction,
) -> Result<BufferHistoryNodeIdentity, Box<dyn Error>>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    let outcome = service.history(0, request, action)?;
    let state = accepted(&outcome)?.result();
    observe(service, request)?;
    Ok(state.history().node())
}

fn observe<M, D, R, S>(
    service: &mut Service<M, D, R, S>,
    request: u128,
) -> Result<(), Box<dyn Error>>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    service.receive(0)?;
    service.receive(1)?;
    let state = service.clients[0]
        .state()
        .ok_or_else(|| io::Error::other("client has no observed state"))?;
    println!(
        "request={} version={} node={:?} client0={:?} client1={:?}",
        request,
        state.version().value(),
        state.history().node(),
        service.clients[0].payload(),
        service.clients[1].payload()
    );
    Ok(())
}

fn accepted<Request: Copy>(
    outcome: &InteractionOutcome<Request>,
) -> Result<InteractionAcceptedState, Box<dyn Error>> {
    outcome.accepted_state().ok_or_else(|| {
        io::Error::other(format!(
            "request was not accepted: {:?}",
            outcome.cause()
        ))
        .into()
    })
}
