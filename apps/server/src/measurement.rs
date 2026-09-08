//! Bounded real-service experiment; stdout contains unfiltered attempt rows.

use std::{
    error::Error,
    io::{self, Write},
};

use reovim_server::assembly;

mod attempt;
mod batch;
mod observation;
mod row;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Copy)]
pub struct Batch {
    pub workload: &'static str,
    pub pair: usize,
    pub warmup: bool,
    pub enabled: bool,
}

/// Executes the declared five warm-up and 100 measured counterbalanced pairs.
pub fn run() -> Result<()> {
    execute(5, 100)
}

/// Explicit validation profile: one pair per workload, no statistical claim.
pub fn run_smoke() -> Result<()> {
    execute(0, 1)
}

fn execute(warmup: usize, measured: usize) -> Result<()> {
    let mut output = io::stdout().lock();
    row::header(&mut output)?;
    for workload in ["heading-small", "heading-limit", "keyed-sparse"] {
        workload_pairs(workload, warmup, measured, &mut output)?;
    }
    Ok(())
}

fn workload_pairs(
    workload: &'static str,
    warmup: usize,
    measured: usize,
    output: &mut impl Write,
) -> Result<()> {
    for pair in 0..warmup + measured {
        let first = pair % 2 != 0;
        run_pair(
            Batch { workload, pair, warmup: pair < warmup, enabled: first },
            output,
        )?;
    }
    Ok(())
}

fn run_pair(mut meta: Batch, output: &mut impl Write) -> Result<()> {
    run_batch(meta, output)?;
    meta.enabled = !meta.enabled;
    run_batch(meta, output)
}

fn run_batch(meta: Batch, output: &mut impl Write) -> Result<()> {
    let observation = observation::Observation::new(meta)?;
    if meta.workload == "keyed-sparse" {
        let service = assembly::keyed(
            observation.owner,
            vec![93, 1],
            observation.telemetry.clone(),
        )?;
        return batch::run(service, meta, &observation, output);
    }
    let mut source = b"# a\n\xffopaque".to_vec();
    if meta.workload == "heading-limit" {
        source.resize(
            usize::try_from(reovim_resource_local::MAX_SOURCE_BYTES)?,
            255,
        );
    }
    let service = assembly::heading(
        observation.owner,
        source,
        observation.telemetry.clone(),
    )?;
    batch::run(service, meta, &observation, output)
}
