use std::io::Write;

use reovim_host_reference::StdClock;
use reovim_kernel_interface::clock::{MonotonicClock, MonotonicInstant};
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcomeCause, InteractionOutcomeStatus,
};
use reovim_kernel_runtime::diagnostics::{
    Telemetry, TelemetryHealth, TraceHealth,
};

use super::{Batch, Result};

pub struct Row {
    pub operation: &'static str,
    pub request: u128,
    pub marks: [Option<MonotonicInstant>; 7],
    pub status: Option<InteractionOutcomeStatus>,
    pub cause: Option<InteractionOutcomeCause>,
    pub oracle: bool,
    pub error: String,
    ring: usize,
    trace: Option<usize>,
    native: Option<u64>,
    health: TelemetryHealth,
    trace_health: Option<TraceHealth>,
}

impl Row {
    pub fn new(operation: &'static str, request: u128) -> Self {
        Self {
            operation,
            request,
            marks: [None; 7],
            status: None,
            cause: None,
            oracle: false,
            error: String::new(),
            ring: 0,
            trace: None,
            native: None,
            health: TelemetryHealth::default(),
            trace_health: None,
        }
    }

    pub fn mark(&mut self, clock: &StdClock, index: usize) -> Result<()> {
        self.marks[index] = Some(clock.observe()?);
        Ok(())
    }

    pub fn charges(&mut self, telemetry: &Telemetry, native: Option<u64>) {
        self.ring = telemetry.resident_bytes();
        self.trace = telemetry.trace_resident_bytes();
        self.native = native;
        self.health = telemetry.health();
        self.trace_health = telemetry.trace_health();
    }

    pub fn write(&self, meta: Batch, output: &mut impl Write) -> Result<()> {
        write!(
            output,
            "{},{},{},{},{},{},{:?},{:?},{},\"{}\"",
            meta.workload,
            meta.pair,
            meta.warmup,
            meta.enabled,
            self.operation,
            self.request,
            self.status,
            self.cause,
            self.oracle,
            self.error.replace('"', "\"\"")
        )?;
        for (first, last) in [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (0, 4),
            (0, 5),
            (0, 6),
        ] {
            write!(output, ",{}", self.duration(first, last)?)?;
        }
        write!(
            output,
            ",{},{},{},{},{},{},{}",
            self.marks.iter().flatten().count(),
            self.ring,
            optional(self.trace),
            optional(self.native),
            self.health.rejected,
            self.health.evicted,
            self.health.output_lost
        )?;
        writeln!(
            output,
            ",{},{},{}",
            optional(self.trace_health.map(|value| value.lost)),
            optional(self.trace_health.map(|value| value.evicted_nodes)),
            optional(self.trace_health.map(|value| value.evicted_snapshots))
        )?;
        Ok(())
    }

    fn duration(&self, first: usize, last: usize) -> Result<String> {
        let (Some(start), Some(end)) = (self.marks[first], self.marks[last])
        else {
            return Ok("NA".into());
        };
        Ok(end.duration_since(start)?.nanoseconds().to_string())
    }
}

pub fn header(output: &mut impl Write) -> Result<()> {
    writeln!(
        output,
        "workload,pair,warmup,enabled,operation,request,status,cause,\
         oracle,error,construction_ns,submission_ns,progress_ns,\
         outcome_ns,update_ns,peer_ns,\
         terminal_total_ns,first_total_ns,peer_total_ns,endpoint_clock_reads,\
         ring_charge_bytes,trace_charge_bytes,native_charge_bytes,\
         ring_rejected,ring_evicted,\
         output_lost,trace_lost,trace_evicted_nodes,trace_evicted_snapshots"
    )?;
    Ok(())
}

fn optional(value: Option<impl core::fmt::Display>) -> String {
    value.map_or_else(|| "NA".into(), |value| value.to_string())
}
