use std::io;

use history_delivery::InteractionHistoryPort;
use reovim_host_reference::StdClock;
use reovim_kernel_interface::interaction::delivery::intent as delivery;
use reovim_kernel_interface::interaction::history::InteractionHistoryAction;
use reovim_kernel_interface::interaction::history_delivery;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDeliveryStatus, BufferChangePort,
};
use reovim_kernel_interface::interaction::outcome::{
    InteractionAcceptedState, InteractionOutcome, InteractionOutcomeCause,
    InteractionOutcomeStatus,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;
use reovim_resource_local::profile;
use reovim_server::assembly::Service;

use super::{Result, row::Row};

pub fn edit<M, D, R, S>(
    service: &mut Service<M, D, R, S>,
    clock: &StdClock,
    row: &mut Row,
    input: &[u8],
    expected: &[u8],
    invalid: bool,
) -> Result<()>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    row.mark(clock, 0)?;
    let intent = service.clients[0].intent(row.request, input)?;
    let lookup = delivery::InteractionOutcomeRequest::new(
        &service.clients[0].session,
        intent.stamp(),
    )?;
    row.mark(clock, 1)?;
    let admitted = service.endpoint(0)?.submit_intent(intent);
    row.mark(clock, 2)?;
    if let Some(refusal) = admitted.non_admission() {
        row.mark(clock, 4)?;
        return complete(
            service,
            clock,
            row,
            &refusal.outcome(),
            expected,
            invalid,
        );
    }
    service.progress()?;
    row.mark(clock, 3)?;
    let outcome = service
        .endpoint(0)?
        .take_outcome(lookup)
        .outcome()
        .ok_or_else(|| io::Error::other("admitted edit has no terminal"))?;
    row.mark(clock, 4)?;
    complete(service, clock, row, &outcome, expected, invalid)
}

pub fn history<M, D, R, S>(
    service: &mut Service<M, D, R, S>,
    clock: &StdClock,
    row: &mut Row,
    action: InteractionHistoryAction,
    expected: &[u8],
) -> Result<()>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    row.mark(clock, 0)?;
    let request = service.clients[0].history(row.request, action)?;
    row.mark(clock, 1)?;
    let admitted = service.endpoint(0)?.submit_history(request);
    row.mark(clock, 2)?;
    if let Some(cause) = admitted.rejection {
        row.mark(clock, 4)?;
        let outcome = InteractionOutcome::not_accepted(request, cause);
        return complete(service, clock, row, &outcome, expected, false);
    }
    service.progress()?;
    row.mark(clock, 3)?;
    let outcome = service
        .endpoint(0)?
        .take_history_outcome(request)
        .outcome()
        .ok_or_else(|| io::Error::other("admitted History has no terminal"))?;
    row.mark(clock, 4)?;
    complete(service, clock, row, &outcome, expected, false)
}

fn complete<M, D, R, S, Request: Copy>(
    service: &mut Service<M, D, R, S>,
    clock: &StdClock,
    row: &mut Row,
    outcome: &InteractionOutcome<Request>,
    expected: &[u8],
    invalid: bool,
) -> Result<()>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    row.status = Some(outcome.status());
    row.cause = outcome.cause();
    let accepted = outcome.status() == InteractionOutcomeStatus::Accepted;
    if accepted {
        service.receive(0)?;
        row.mark(clock, 5)?;
        service.receive(1)?;
        row.mark(clock, 6)?;
    }
    let observed = service.clients[0].state();
    let expected_state =
        outcome.accepted_state().map(InteractionAcceptedState::result);
    row.oracle = accepted != invalid
        && (!invalid
            || outcome.cause() == Some(InteractionOutcomeCause::PolicyFailure))
        && (!accepted || observed == expected_state)
        && observed == service.clients[1].state()
        && service
            .clients
            .iter()
            .all(|client| client.payload() == Some(expected));
    if invalid {
        row.oracle &= unchanged(service)?;
    }
    if !row.oracle {
        return Err(
            io::Error::other("independent semantic oracle failed").into()
        );
    }
    Ok(())
}

fn unchanged<M, D, R, S>(service: &mut Service<M, D, R, S>) -> Result<bool>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    let installed = service.clients[0].state();
    let port = service.endpoint(0)?;
    let subscription = port.subscribe_changes(profile::count(1))?;
    let delivery =
        port.take_change(&subscription, subscription.checkpoint.version());
    port.unsubscribe_changes(&subscription)?;
    Ok(installed == Some(subscription.checkpoint)
        && delivery?.status() == BufferChangeDeliveryStatus::Pending)
}
