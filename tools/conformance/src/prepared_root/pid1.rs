//! Namespace PID-1 setup, bounded workload execution, and final observation.

use std::{
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use nix::{
    mount::{MsFlags, mount},
    sys::{
        prctl::{get_pdeathsig, set_pdeathsig},
        resource::{Resource, getrlimit, setrlimit},
        signal::Signal,
    },
    unistd::{chdir, chroot},
};
use serde_json::Value;

use crate::{digest, json as bounded_json};

use super::{
    linux::{PROC_PATH, SCRATCH_PATH, prepared_path},
    model::{Config, Error, Pid1Result, WorkloadContext, WorkloadObservation},
    network, observation, protocol, reaper,
};

const WORKLOAD_DOMAIN: &[u8] = b"reovim-prepared-workload-result/v1\0";

pub(super) fn run(
    config: &Config,
    workload: fn(&WorkloadContext) -> Result<Value, Error>,
) -> Result<(), Error> {
    let pdeath = arm_parent_death_signal()?;
    mount_proc(config)?;
    enter_root(config)?;
    apply_resource_limits(config)?;
    let namespaces = observation::namespaces(config, pdeath)?;
    observation::verify_mounts(config)?;
    let network_before = network::observe(config.dispatch_bounds)?;
    write_ready()?;

    let context = workload_context(config)?;
    let workload_value = workload(&context);
    let descendants_reaped = reaper::terminate_and_reap(
        config.descendant_reap_limit,
        config.dispatch_bounds.execution_deadline,
    )?;
    let network_after = network::observe(config.dispatch_bounds)?;
    if network_after != network_before {
        return Err(Error::new(
            "PREPARED_NETWORK_DRIFT",
            "network state changed during the workload",
        ));
    }
    observation::verify_mounts(config)?;
    verify_resource_limits(config)?;
    let scratch = observation::scratch(config)?;
    let workload = observe_workload(workload_value?, config)?;
    let result = Pid1Result {
        namespaces,
        network_before,
        network_after,
        scratch,
        workload,
        descendants_reaped,
    };
    write_result(&result, config)
}

fn arm_parent_death_signal() -> Result<bool, Error> {
    set_pdeathsig(Signal::SIGKILL).map_err(|failure| {
        Error::new("PREPARED_PDEATH_SET", failure.to_string())
    })?;
    let observed = get_pdeathsig().map_err(|failure| {
        Error::new("PREPARED_PDEATH_GET", failure.to_string())
    })?;
    if observed == Some(Signal::SIGKILL) {
        return Ok(true);
    }
    Err(Error::new(
        "PREPARED_PDEATH_IDENTITY",
        "parent-death signal differs from SIGKILL",
    ))
}

fn mount_proc(config: &Config) -> Result<(), Error> {
    let target = prepared_path(&config.mountpoint, Path::new(PROC_PATH))?;
    let flags = MsFlags::MS_RDONLY
        | MsFlags::MS_NOSUID
        | MsFlags::MS_NODEV
        | MsFlags::MS_NOEXEC;
    mount(Some("proc"), &target, Some("proc"), flags, None::<&str>).map_err(
        |failure| Error::new("PREPARED_PROC_MOUNT", failure.to_string()),
    )
}

fn enter_root(config: &Config) -> Result<(), Error> {
    chroot(&config.mountpoint).map_err(|failure| {
        Error::new("PREPARED_CHROOT", failure.to_string())
    })?;
    chdir("/")
        .map_err(|failure| Error::new("PREPARED_CHDIR", failure.to_string()))?;
    let current = std::env::current_dir().map_err(|failure| {
        Error::new("PREPARED_CHDIR_VERIFY", failure.to_string())
    })?;
    if current == Path::new("/") {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_CHDIR_VERIFY",
        "PID 1 did not enter the prepared root directory",
    ))
}

fn apply_resource_limits(config: &Config) -> Result<(), Error> {
    let limits = config.resource_limits;
    for (resource, value, name) in [
        (Resource::RLIMIT_AS, limits.address_space_bytes, "address_space"),
        (Resource::RLIMIT_CPU, limits.cpu_seconds, "cpu"),
        (Resource::RLIMIT_NOFILE, limits.open_files, "open_files"),
        (Resource::RLIMIT_FSIZE, limits.output_file_bytes, "file_size"),
        (Resource::RLIMIT_CORE, limits.core_dump_bytes, "core_dump"),
    ] {
        setrlimit(resource, value, value).map_err(|failure| {
            Error::at("PREPARED_RLIMIT_SET", name, failure.to_string())
        })?;
    }
    verify_resource_limits(config)
}

fn verify_resource_limits(config: &Config) -> Result<(), Error> {
    let limits = config.resource_limits;
    for (resource, expected, name) in [
        (Resource::RLIMIT_AS, limits.address_space_bytes, "address_space"),
        (Resource::RLIMIT_CPU, limits.cpu_seconds, "cpu"),
        (Resource::RLIMIT_NOFILE, limits.open_files, "open_files"),
        (Resource::RLIMIT_FSIZE, limits.output_file_bytes, "file_size"),
        (Resource::RLIMIT_CORE, limits.core_dump_bytes, "core_dump"),
    ] {
        let observed = getrlimit(resource).map_err(|failure| {
            Error::at("PREPARED_RLIMIT_GET", name, failure.to_string())
        })?;
        if observed != (expected, expected) {
            return Err(Error::at(
                "PREPARED_RLIMIT_IDENTITY",
                name,
                format!("observed {observed:?}, expected {expected}"),
            ));
        }
    }
    Ok(())
}

fn workload_context(config: &Config) -> Result<WorkloadContext, Error> {
    let repository_files = config.exposures.iter().filter(|value| {
        value.plan.source.starts_with(&config.repository_root)
            && value.plan.destination.starts_with("/repo")
            && value.plan.kind == super::model::ExposureKind::File
    });
    if repository_files.count() == 0 {
        return Err(Error::new(
            "PREPARED_REPOSITORY_EXPOSURE",
            "prepared repository file set is empty",
        ));
    }
    Ok(WorkloadContext {
        scratch_root: PathBuf::from(SCRATCH_PATH),
        repository_root: Some(PathBuf::from("/repo")),
        workload_request: config.workload_request.clone(),
    })
}

pub(super) fn observe_workload(
    value: Value,
    config: &Config,
) -> Result<WorkloadObservation, Error> {
    let bytes = serde_json::to_vec(&value).map_err(|failure| {
        Error::new("PREPARED_WORKLOAD_ENCODE", failure.to_string())
    })?;
    if bytes.len() > config.workload_result_bytes {
        return Err(Error::new(
            "PREPARED_WORKLOAD_BYTE_LIMIT",
            "workload result exceeds its byte limit",
        ));
    }
    let parsed = bounded_json::parse(
        &bytes,
        config.dispatch_bounds.json_depth,
        config.dispatch_bounds.json_items,
    )
    .map_err(|failure| {
        Error::new("PREPARED_WORKLOAD_JSON", failure.to_string())
    })?;
    if parsed != value {
        return Err(Error::new(
            "PREPARED_WORKLOAD_ROUNDTRIP",
            "workload JSON changes across canonical serialization",
        ));
    }
    let bytes_count = u64::try_from(bytes.len()).map_err(|_| {
        Error::new(
            "PREPARED_WORKLOAD_LENGTH",
            "workload result length does not fit u64",
        )
    })?;
    let mut digest_input =
        Vec::with_capacity(WORKLOAD_DOMAIN.len().saturating_add(bytes.len()));
    digest_input.extend_from_slice(WORKLOAD_DOMAIN);
    digest_input.extend_from_slice(&bytes);
    Ok(WorkloadObservation {
        status: "passed".to_owned(),
        bytes: bytes_count,
        digest: digest::bytes(&digest_input),
        value,
    })
}

fn write_ready() -> Result<(), Error> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    output.write_all(protocol::ready_frame()).map_err(|failure| {
        Error::new("PREPARED_HANDSHAKE_WRITE", failure.to_string())
    })?;
    output.flush().map_err(|failure| {
        Error::new("PREPARED_HANDSHAKE_FLUSH", failure.to_string())
    })
}

fn write_result(result: &Pid1Result, config: &Config) -> Result<(), Error> {
    let mut bytes =
        protocol::encode_pid1_result(result, config.dispatch_bounds)?;
    bytes.push(b'\n');
    let stdout = io::stdout();
    let mut output = stdout.lock();
    output.write_all(&bytes).map_err(|failure| {
        Error::new("PREPARED_RESULT_WRITE", failure.to_string())
    })?;
    output.flush().map_err(|failure| {
        Error::new("PREPARED_RESULT_FLUSH", failure.to_string())
    })
}
