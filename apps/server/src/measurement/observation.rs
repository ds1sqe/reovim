use std::{
    io::{self, Write},
    rc::Rc,
};

use reovim_host_reference::{DiagnosticQueue, StdClock};
use reovim_kernel_interface::clock::{ClockDescriptor, MonotonicClock};
use reovim_kernel_interface::event::trace::{
    TraceCut, TraceLimits, TraceReadPort, TraceStart,
};
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::session::InteractionSessionHandle;
use reovim_kernel_interface::lifecycle::{LifecycleIdentity, ShutdownOutcome};
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;
use reovim_server::assembly::life;
use reovim_server::driver::DriverError;

use super::{Batch, Result};

pub struct Observation {
    pub owner: LifecycleIdentity,
    pub client_clock: LifecycleIdentity,
    pub telemetry: Rc<Telemetry>,
    stream: LifecycleIdentity,
    grant: LifecycleIdentity,
    output: Rc<DiagnosticQueue>,
    kernel_clock: Rc<StdClock>,
}

impl Observation {
    pub fn new(meta: Batch) -> Result<Self> {
        let owner = owner(meta)?;
        let base = owner.instance() * 1000;
        let stream = life(base + 800);
        let grant = life(base + 803);
        let client_clock = life(base + 804);
        let kernel_clock = Rc::new(clock(owner, life(base + 801))?);
        let output = Rc::new(DiagnosticQueue::new(
            owner,
            life(base + 802),
            stream,
            256,
            2048,
            512 * 1024,
        )?);
        let telemetry = Rc::new(
            Telemetry::new(
                owner,
                stream,
                kernel_clock.clone(),
                Some(output.clone()),
                256,
            )?
            .with_tracing(
                grant,
                TraceLimits {
                    nodes: profile::count(256),
                    snapshots: profile::count(64),
                    metadata_bytes: profile::bytes(1024 * 1024),
                    payload_bytes: profile::bytes(64 * 1024),
                },
            )?,
        );
        telemetry.set_enabled(meta.enabled);
        Ok(Self {
            owner,
            client_clock,
            telemetry,
            stream,
            grant,
            output,
            kernel_clock,
        })
    }

    pub fn finish(
        &self,
        meta: Batch,
        client_clock: &StdClock,
        origin: InteractionSessionHandle,
        service_close: std::result::Result<(), DriverError>,
    ) -> Result<()> {
        let calibration = calibrate(client_clock)?;
        let start = client_clock.observe()?;
        let report = self.telemetry.backtrace(
            self.grant,
            TraceStart::Request {
                session: origin,
                request: InteractionRequestIdentity::new(12),
            },
            crate::trace_report::budget(),
        );
        let query_ns =
            client_clock.observe()?.duration_since(start)?.nanoseconds();
        let start = client_clock.observe()?;
        self.telemetry.close();
        self.telemetry.flush();
        let mut stderr = io::stderr().lock();
        while let Some(record) = self.output.take()? {
            stderr.write_all(record.bytes())?;
        }
        stderr.flush()?;
        let export_ns =
            client_clock.observe()?.duration_since(start)?.nanoseconds();
        let closed = self.output.close()?;
        writeln!(
            stderr,
            "measurement-batch workload={} pair={} warmup={} enabled={} \
             query_ns={} export_ns={} trace_cut={} calibration_ns={:?} \
             telemetry={:?} trace={:?} output_close={:?} service_close={:?}",
            meta.workload,
            meta.pair,
            meta.warmup,
            meta.enabled,
            query_ns,
            export_ns,
            report.as_ref().map_or_else(
                |cause| format!("Error({cause})"),
                |report| format!("{:?}", report.cut)
            ),
            calibration,
            self.telemetry.health(),
            self.telemetry.trace_health(),
            closed,
            service_close
        )?;
        writeln!(
            stderr,
            "measurement-identities workload={} pair={} enabled={} \
             root={:?} stream={:?} diagnostic_clock={:?} client_clock={:?} \
             grant={:?} \
             origin={:?}",
            meta.workload,
            meta.pair,
            meta.enabled,
            self.owner,
            self.stream,
            self.kernel_clock.descriptor().lifecycle(),
            self.client_clock,
            self.grant,
            origin
        )?;
        self.kernel_clock.close();
        if closed != ShutdownOutcome::Closed {
            return Err(
                io::Error::other("diagnostic ownership incomplete").into()
            );
        }
        let expected = if meta.enabled {
            TraceCut::Complete
        } else {
            TraceCut::NotCaptured
        };
        if report?.cut != expected {
            return Err(io::Error::other("unexpected trace query cut").into());
        }
        Ok(())
    }
}

fn owner(meta: Batch) -> Result<LifecycleIdentity> {
    let prefix = match meta.workload {
        "heading-small" => 10000,
        "heading-limit" => 20000,
        "keyed-sparse" => 30000,
        _ => return Err(io::Error::other("undeclared workload").into()),
    };
    let batch = u128::try_from(meta.pair)? * 2 + u128::from(meta.enabled);
    Ok(life(prefix + batch))
}

pub fn clock(
    owner: LifecycleIdentity,
    identity: LifecycleIdentity,
) -> Result<StdClock> {
    Ok(StdClock::new(ClockDescriptor::new(
        profile::schema(0x8001),
        identity,
        owner,
        1,
        u64::MAX,
        0x8002,
    )?))
}

fn calibrate(clock: &StdClock) -> Result<Vec<u64>> {
    let mut samples = Vec::with_capacity(32);
    for _ in 0..32 {
        let first = clock.observe()?;
        let second = clock.observe()?;
        samples.push(second.duration_since(first)?.nanoseconds());
    }
    Ok(samples)
}
