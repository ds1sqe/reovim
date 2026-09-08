use std::io::Write;

use reovim_host_reference::StdClock;
use reovim_kernel_interface::interaction::history::InteractionHistoryAction;
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;
use reovim_server::assembly::Service;

use super::{
    Batch, Result, attempt,
    observation::{self, Observation},
    row::Row,
};

pub fn run<M, D, R, S>(
    mut service: Service<M, D, R, S>,
    meta: Batch,
    observation: &Observation,
    output: &mut impl Write,
) -> Result<()>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    service.open(0)?;
    service.open(1)?;
    let origin = service.clients[0].session.stamp();
    let clock = observation::clock(
        origin.client().lifecycle(),
        observation.client_clock,
    )?;
    let result = schedule(&mut service, &clock, meta, output);
    let closed = service.close();
    output.flush()?;
    let exported = observation.finish(meta, &clock, origin.handle(), closed);
    clock.close();
    result?;
    closed?;
    exported
}

fn schedule<M, D, R, S>(
    service: &mut Service<M, D, R, S>,
    clock: &StdClock,
    meta: Batch,
    output: &mut impl Write,
) -> Result<()>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    let keyed = meta.workload == "keyed-sparse";
    let mut before =
        if keyed { vec![b'K', 93, 1] } else { vec![b'H', 1, b'a'] };
    for cycle in 0..4 {
        let input = if keyed { 93 } else { b'b' + cycle };
        let mut after = before.clone();
        after[2] = if keyed { before[2] ^ 1 } else { input };
        cycle_attempts(
            service,
            clock,
            meta,
            &Cycle {
                request: u128::from(cycle) * 3 + 1,
                input,
                before: &before,
                after: &after,
            },
            output,
        )?;
        before = after;
    }
    let input = if keyed { 17 } else { b'?' };
    let mut row = Row::new("invalid-edit", 13);
    let result =
        attempt::edit(service, clock, &mut row, &[input], &before, true);
    record(service, row, result, meta, output)
}

struct Cycle<'a> {
    request: u128,
    input: u8,
    before: &'a [u8],
    after: &'a [u8],
}

fn cycle_attempts<M, D, R, S>(
    service: &mut Service<M, D, R, S>,
    clock: &StdClock,
    meta: Batch,
    cycle: &Cycle<'_>,
    output: &mut impl Write,
) -> Result<()>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    let mut row = Row::new("edit", cycle.request);
    let result = attempt::edit(
        service,
        clock,
        &mut row,
        &[cycle.input],
        cycle.after,
        false,
    );
    record(service, row, result, meta, output)?;
    let mut row = Row::new("undo", cycle.request + 1);
    let result = attempt::history(
        service,
        clock,
        &mut row,
        InteractionHistoryAction::Undo,
        cycle.before,
    );
    record(service, row, result, meta, output)?;
    let mut row = Row::new("redo", cycle.request + 2);
    let result = attempt::history(
        service,
        clock,
        &mut row,
        InteractionHistoryAction::Redo { child: None },
        cycle.after,
    );
    record(service, row, result, meta, output)
}

fn record<M, D, R, S>(
    service: &Service<M, D, R, S>,
    mut row: Row,
    result: Result<()>,
    meta: Batch,
    output: &mut impl Write,
) -> Result<()>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    if let Err(cause) = &result {
        row.error = cause.to_string();
    }
    row.charges(
        &service.telemetry,
        service
            .runtime
            .retained_native_bytes()
            .map(reovim_kernel_interface::bound::CheckedByteCount::value),
    );
    row.write(meta, output)?;
    result
}
