//! Authorized debug consumer of actual retained causal/semantic evidence.

use std::{
    error::Error,
    io::{self, Write},
};

use reovim_kernel_interface::cancellation::DeadlineState;
use reovim_kernel_interface::event::trace::{
    SemanticCaptureKey, TraceQueryBudget, TraceReadPort, TraceStart,
};
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::session::InteractionSessionHandle;
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;

pub fn show(
    telemetry: &Telemetry,
    grant: LifecycleIdentity,
    origin: InteractionSessionHandle,
    detailed: bool,
) -> Result<(), Box<dyn Error>> {
    let report = telemetry.backtrace(
        grant,
        TraceStart::Request {
            session: origin,
            request: InteractionRequestIdentity::new(55),
        },
        budget(),
    )?;
    let mut output = io::stdout().lock();
    writeln!(
        output,
        "trace request=55 cut={:?} oldest={:?} health={:?}",
        report.cut,
        report.oldest,
        telemetry.trace_health()
    )?;
    if !detailed {
        return Ok(());
    }
    for node in report.nodes {
        writeln!(output, "trace-node {node:?}")?;
        if let (Some(update), Some(state)) =
            (node.context.update, node.context.state)
        {
            let captured = telemetry.captured(
                grant,
                &SemanticCaptureKey { update, state },
                profile::bytes(8192),
            );
            writeln!(output, "semantic-capture {captured:?}")?;
        }
    }
    for gap in report.gaps {
        writeln!(output, "trace-gap {gap:?}")?;
    }
    Ok(())
}

pub fn budget() -> TraceQueryBudget {
    TraceQueryBudget {
        primary: true,
        joins: true,
        nodes: profile::count(128),
        edges: profile::count(256),
        depth: profile::count(32),
        work: profile::count(65536),
        bytes: profile::bytes(512 * 1024),
        deadline: DeadlineState::Pending,
    }
}
