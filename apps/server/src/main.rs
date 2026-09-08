//! Hosted local smoke: no protocol, renderer or fixture acceptance hooks.

use std::{
    error::Error,
    io::{self, Write},
    rc::Rc,
};

use reovim_host_reference::{DiagnosticQueue, StdClock};
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::event::trace::TraceLimits;
use reovim_kernel_interface::lifecycle::ShutdownOutcome;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;
use reovim_server::assembly::{self, life};

mod measurement;
mod scenario;
mod trace_report;

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let scenario = arguments.next().unwrap_or_else(|| "both".into());
    let option = arguments.next();
    if option.is_none() && scenario == "--measure" {
        return measurement::run();
    }
    if option.is_none() && scenario == "--measure-smoke" {
        return measurement::run_smoke();
    }
    let detailed = option.as_deref() == Some("--trace");
    if (option.is_some() && !detailed)
        || arguments.next().is_some()
        || !matches!(scenario.as_str(), "heading" | "keyed" | "both")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            concat!(
                "usage: reovim-server [heading|keyed|both] [--trace]; ",
                "or reovim-server --measure[-smoke]",
            ),
        )
        .into());
    }
    if scenario != "keyed" {
        heading(detailed)?;
    }
    if scenario != "heading" {
        keyed(detailed)?;
    }
    Ok(())
}

fn heading(detailed: bool) -> Result<(), Box<dyn Error>> {
    let (telemetry, output, clock) = diagnostics(1)?;
    let service = assembly::heading(
        life(1),
        b"# a\n\xffopaque".to_vec(),
        Rc::clone(&telemetry),
    )?;
    let origin = service.clients[0].session.stamp().handle();
    scenario::exercise(service, &[b"b", b"c", b"d", b"e"], b"z")?;
    trace_report::show(&telemetry, trace_grant(1), origin, detailed)?;
    export(&telemetry, &output)?;
    clock.close();
    Ok(())
}

fn keyed(detailed: bool) -> Result<(), Box<dyn Error>> {
    let (telemetry, output, clock) = diagnostics(2)?;
    let service =
        assembly::keyed(life(2), vec![17, 0, 93, 1], Rc::clone(&telemetry))?;
    let origin = service.clients[0].session.stamp().handle();
    scenario::exercise(service, &[&[17], &[17], &[93], &[17]], &[93])?;
    trace_report::show(&telemetry, trace_grant(2), origin, detailed)?;
    export(&telemetry, &output)?;
    clock.close();
    Ok(())
}

type Diagnostics = (Rc<Telemetry>, Rc<DiagnosticQueue>, Rc<StdClock>);

fn diagnostics(root: u128) -> Result<Diagnostics, Box<dyn Error>> {
    let owner = life(root);
    let stream = life(9000 + root * 10);
    let clock = Rc::new(StdClock::new(ClockDescriptor::new(
        profile::schema(0x8001),
        life(9001 + root * 10),
        owner,
        1,
        u64::MAX,
        0x8002,
    )?));
    let output = Rc::new(DiagnosticQueue::new(
        owner,
        life(9002 + root * 10),
        stream,
        256,
        2048,
        512 * 1024,
    )?);
    let telemetry = Rc::new(
        Telemetry::new(
            owner,
            stream,
            clock.clone(),
            Some(output.clone()),
            256,
        )?
        .with_tracing(
            trace_grant(root),
            TraceLimits {
                nodes: profile::count(256),
                snapshots: profile::count(64),
                metadata_bytes: profile::bytes(1024 * 1024),
                payload_bytes: profile::bytes(64 * 1024),
            },
        )?,
    );
    Ok((telemetry, output, clock))
}

const fn trace_grant(
    root: u128,
) -> reovim_kernel_interface::lifecycle::LifecycleIdentity {
    life(9003 + root * 10)
}

fn export(
    telemetry: &Telemetry,
    output: &DiagnosticQueue,
) -> Result<(), Box<dyn Error>> {
    telemetry.close();
    telemetry.flush();
    let mut stderr = io::stderr().lock();
    while let Some(record) = output.take()? {
        stderr.write_all(record.bytes())?;
    }
    if output.close()? != ShutdownOutcome::Closed {
        return Err(io::Error::other("diagnostic ownership incomplete").into());
    }
    writeln!(stderr, "telemetry_health={:?}", telemetry.health())?;
    Ok(())
}
