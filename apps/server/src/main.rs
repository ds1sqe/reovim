//! Hosted local smoke: no protocol, renderer or fixture acceptance hooks.

use std::{
    error::Error,
    io::{self, Write},
    rc::Rc,
};

use reovim_host_reference::{DiagnosticQueue, StdClock};
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::lifecycle::ShutdownOutcome;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;
use reovim_server::assembly::{self, life};

mod scenario;

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let scenario = arguments.next().unwrap_or_else(|| "both".into());
    if arguments.next().is_some()
        || !matches!(scenario.as_str(), "heading" | "keyed" | "both")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: reovim-server [heading|keyed|both]",
        )
        .into());
    }
    if scenario != "keyed" {
        heading()?;
    }
    if scenario != "heading" {
        keyed()?;
    }
    Ok(())
}

fn heading() -> Result<(), Box<dyn Error>> {
    let (telemetry, output, clock) = diagnostics(1)?;
    let service = assembly::heading(
        life(1),
        b"# a\n\xffopaque".to_vec(),
        Rc::clone(&telemetry),
    )?;
    scenario::exercise(service, &[b"b", b"c", b"d", b"e"], b"z")?;
    export(&telemetry, &output)?;
    clock.close();
    Ok(())
}

fn keyed() -> Result<(), Box<dyn Error>> {
    let (telemetry, output, clock) = diagnostics(2)?;
    let service =
        assembly::keyed(life(2), vec![17, 0, 93, 1], Rc::clone(&telemetry))?;
    scenario::exercise(service, &[&[17], &[17], &[93], &[17]], &[93])?;
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
        64,
        2048,
        128 * 1024,
    )?);
    let telemetry = Rc::new(Telemetry::new(
        owner,
        stream,
        clock.clone(),
        Some(output.clone()),
        64,
    )?);
    Ok((telemetry, output, clock))
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
