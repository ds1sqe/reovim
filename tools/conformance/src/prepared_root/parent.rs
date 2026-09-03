//! Parent-side prepared-root ownership, supervision, and evidence acceptance.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::PathBuf,
};

use crate::process::{
    self, CommandSpec, ProcessLimits, ProcessResult, TerminationReason,
};

use super::{
    config_transport, diagnostic,
    linux::{
        ParentMountpoint, configure_exposures, current_ids, namespace_links,
        one_final_line, require_configured_identities, validate_plan,
    },
    model::{
        CleanupObservation, Config, ConfiguredExposure, Error, Evidence,
        Execution, ExposureKind, ExposureObservation, NamespaceObservation,
        Plan, SetupResult,
    },
    network, pid1, protocol,
};

#[cfg(test)]
use super::model::ConfigTransportInspection;

pub(super) fn execute(plan: &Plan) -> Result<Execution, Error> {
    let (config, mountpoint) = assemble_config(plan)?;
    let journey = run_outer(&config);
    let post_identity = require_configured_identities(&config);
    let mount_cleanup = mountpoint.finish();
    mount_cleanup?;
    post_identity?;
    let (process, setup) = journey?;
    let evidence = evidence(plan, &setup, &process)?;
    Ok(Execution { evidence, process })
}

#[cfg(test)]
pub(super) fn inspect_config_transport(
    plan: &Plan,
) -> Result<ConfigTransportInspection, Error> {
    let (config, mountpoint) = assemble_config(plan)?;
    let inspection = config_transport::encode(&config).map(|value| value.1);
    let post_identity = require_configured_identities(&config);
    let mount_cleanup = mountpoint.finish();
    mount_cleanup?;
    post_identity?;
    inspection
}

fn assemble_config(plan: &Plan) -> Result<(Config, ParentMountpoint), Error> {
    validate_plan(plan)?;
    let exposures = configure_exposures(plan)?;
    let parent_namespaces = namespace_links()?;
    let host_identity = current_ids()?;
    let mountpoint = ParentMountpoint::create(plan)?;
    let config = Config {
        mountpoint: mountpoint.path().to_path_buf(),
        repository_root: plan.repository_root.clone(),
        execution_environment: plan.execution_environment.clone(),
        workload_request: plan.workload_request.clone(),
        checker_executable: plan.checker_executable.clone(),
        exposures,
        inventory_limits: plan.inventory_limits,
        scratch_limits: plan.scratch_limits,
        resource_limits: plan.resource_limits,
        dispatch_bounds: plan.dispatch_bounds,
        workload_result_bytes: plan.workload_result_bytes,
        descendant_reap_limit: plan.descendant_reap_limit,
        parent_namespaces,
        outside_uid: host_identity.0,
        outside_gid: host_identity.1,
    };
    Ok((config, mountpoint))
}

fn run_outer(config: &Config) -> Result<(ProcessResult, SetupResult), Error> {
    let config_bytes = setup_environment_payload(config)?;
    let config_text =
        std::str::from_utf8(&config_bytes).map_err(|failure| {
            Error::new("PREPARED_CONFIG_UTF8", failure.to_string())
        })?;
    let mut environment = BTreeMap::new();
    environment.insert(
        OsString::from(super::model::CONFIG_ENVIRONMENT),
        OsString::from(config_text),
    );
    environment.insert(OsString::from("TERM"), OsString::from("dumb"));
    let spec = CommandSpec {
        program: config.checker_executable.clone(),
        arguments: vec![OsString::from(super::model::SETUP_ARGUMENT)],
        current_dir: PathBuf::from("/"),
        environment,
        clear_environment: true,
    };
    let limits = ProcessLimits {
        stdout_bytes: config.dispatch_bounds.result_bytes,
        stderr_bytes: config.dispatch_bounds.result_bytes,
        deadline: config.dispatch_bounds.execution_deadline,
        supervision_deadline: config.dispatch_bounds.supervision_deadline,
    };
    let process = process::run(&spec, limits).map_err(|failure| {
        Error::new("PREPARED_SETUP_PROCESS", failure.to_string())
    })?;
    validate_process(&process, config)?;
    let result = one_final_line(&process.stdout, "PREPARED_SETUP_RESULT")?;
    let result = result.to_vec();
    let setup = protocol::parse_setup_result(&result, config.dispatch_bounds)?;
    validate_setup(&setup, config)?;
    Ok((process, setup))
}

fn setup_environment_payload(config: &Config) -> Result<Vec<u8>, Error> {
    config_transport::encode(config).map(|value| value.0)
}

#[cfg(test)]
pub(super) fn setup_environment_payload_for_test(
    config: &Config,
) -> Result<Vec<u8>, Error> {
    setup_environment_payload(config)
}

fn validate_process(
    result: &ProcessResult,
    config: &Config,
) -> Result<(), Error> {
    validate_termination(result)?;
    validate_streams(result)?;
    validate_child_status(result, config)?;
    validate_argv(result, config)?;
    validate_process_ownership(result)
}

fn validate_termination(result: &ProcessResult) -> Result<(), Error> {
    match result.termination_reason {
        TerminationReason::Exited => Ok(()),
        TerminationReason::Deadline => Err(Error::new(
            "PREPARED_SETUP_DEADLINE",
            format!(
                "setup exceeded its execution deadline; status={}",
                result.status
            ),
        )),
        reason => Err(Error::new(
            "PREPARED_SETUP_TERMINATION",
            format!(
                "setup terminated for {}; status={}",
                reason.as_str(),
                result.status
            ),
        )),
    }
}

fn validate_child_status(
    result: &ProcessResult,
    config: &Config,
) -> Result<(), Error> {
    if !result.stderr.is_empty() {
        let summary = diagnostic::child_error_summary(
            &result.stderr,
            config.dispatch_bounds,
        );
        let code = if result.status.code() == Some(0) {
            "PREPARED_SETUP_UNEXPECTED_STDERR"
        } else {
            "PREPARED_SETUP_CHILD_FAILURE"
        };
        return Err(Error::new(
            code,
            format!("status={}; {summary}", result.status),
        ));
    }
    if result.status.code() == Some(0) {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_SETUP_EXIT_STATUS",
        format!("setup exited without a diagnostic; status={}", result.status),
    ))
}

fn validate_streams(result: &ProcessResult) -> Result<(), Error> {
    let stdout_len = retained_length(result.stdout.len(), "stdout")?;
    let stderr_len = retained_length(result.stderr.len(), "stderr")?;
    if result.stdout_overflow
        || result.stderr_overflow
        || result.limit_observed_after_exit
    {
        return Err(Error::new(
            "PREPARED_SETUP_STREAM_LIMIT",
            format!(
                "stdout_total={}, stdout_retained={}, stderr_total={}, \
                 stderr_retained={}, after_exit={}",
                result.stdout_bytes,
                stdout_len,
                result.stderr_bytes,
                stderr_len,
                result.limit_observed_after_exit
            ),
        ));
    }
    if result.stdout_read_error.is_some() || result.stderr_read_error.is_some()
    {
        return Err(stream_read_error(result));
    }
    if result.stdout_bytes == stdout_len && result.stderr_bytes == stderr_len {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_SETUP_STREAM_ACCOUNTING",
        format!(
            "stdout_total={}, stdout_retained={}, stderr_total={}, \
             stderr_retained={}",
            result.stdout_bytes, stdout_len, result.stderr_bytes, stderr_len
        ),
    ))
}

fn retained_length(length: usize, name: &str) -> Result<u64, Error> {
    u64::try_from(length).map_err(|_| {
        Error::new(
            "PREPARED_SETUP_STREAM_ACCOUNTING",
            format!("retained {name} length does not fit u64"),
        )
    })
}

fn stream_read_error(result: &ProcessResult) -> Error {
    let stdout = result
        .stdout_read_error
        .as_deref()
        .map_or_else(|| "none".to_owned(), diagnostic::redact_diagnostic);
    let stderr = result
        .stderr_read_error
        .as_deref()
        .map_or_else(|| "none".to_owned(), diagnostic::redact_diagnostic);
    Error::new(
        "PREPARED_SETUP_STREAM_READ",
        format!("stdout={stdout}; stderr={stderr}"),
    )
}

fn validate_argv(result: &ProcessResult, config: &Config) -> Result<(), Error> {
    let expected = [
        config.checker_executable.as_os_str(),
        std::ffi::OsStr::new(super::model::SETUP_ARGUMENT),
    ];
    if result.argv.iter().map(OsString::as_os_str).eq(expected) {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_SETUP_ARGV_IDENTITY",
        "setup argv differs from the exact checker child invocation",
    ))
}

fn validate_process_ownership(result: &ProcessResult) -> Result<(), Error> {
    if result.pgid != Some(result.leader_pid)
        || !result.group_identity_validated
        || !result.group_absent_after_reap
    {
        return Err(Error::new(
            "PREPARED_SETUP_PROCESS_GROUP",
            format!(
                "identity_validated={}, group_absent={}",
                result.group_identity_validated, result.group_absent_after_reap
            ),
        ));
    }
    if !result.descendants_owned
        || !result.descendants_reaped
        || result.descendant_residue_observed
    {
        return Err(Error::new(
            "PREPARED_SETUP_DESCENDANTS",
            format!(
                "owned={}, reaped={}, residue={}",
                result.descendants_owned,
                result.descendants_reaped,
                result.descendant_residue_observed
            ),
        ));
    }
    if !result.reaped {
        return Err(Error::new(
            "PREPARED_SETUP_LEADER_REAP",
            "setup process leader was not reaped",
        ));
    }
    if result.readers_joined {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_SETUP_READERS",
        "setup pipe readers were not joined",
    ))
}

fn validate_setup(result: &SetupResult, config: &Config) -> Result<(), Error> {
    validate_exposures(&result.exposures, &config.exposures)?;
    validate_namespaces(&result.pid1.namespaces, config)?;
    network::validate_observation(&result.pid1.network_before)?;
    network::validate_observation(&result.pid1.network_after)?;
    if result.pid1.network_before != result.pid1.network_after {
        return Err(Error::new(
            "PREPARED_NETWORK_DRIFT",
            "retained network observations differ",
        ));
    }
    validate_scratch(result, config)?;
    let workload =
        pid1::observe_workload(result.pid1.workload.value.clone(), config)?;
    let cleanup = result.cleanup;
    let complete = workload == result.pid1.workload
        && result.pid1.descendants_reaped
        && cleanup.readers_joined()
        && cleanup.proc_unmounted()
        && cleanup.exposures_unmounted()
        && cleanup.scratch_unmounted()
        && cleanup.source_identity_preserved()
        && cleanup.tool_identity_preserved();
    if complete {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_SETUP_EVIDENCE",
        "setup result or cleanup evidence is incomplete",
    ))
}

fn validate_exposures(
    observed: &[ExposureObservation],
    expected: &[ConfiguredExposure],
) -> Result<(), Error> {
    if observed.len() != expected.len() {
        return Err(Error::new(
            "PREPARED_EXPOSURE_JOIN",
            "mounted exposure count differs from the plan",
        ));
    }
    let mut ids = BTreeSet::new();
    for value in observed {
        if !ids.insert(value.id.as_str()) {
            return Err(Error::new(
                "PREPARED_EXPOSURE_JOIN",
                "mounted exposure IDs are duplicated",
            ));
        }
        let planned = expected
            .iter()
            .find(|item| item.plan.id == value.id)
            .ok_or_else(|| {
                Error::new(
                    "PREPARED_EXPOSURE_JOIN",
                    format!("unknown mounted exposure {}", value.id),
                )
            })?;
        validate_exposure(value, planned)?;
    }
    Ok(())
}

fn validate_exposure(
    value: &ExposureObservation,
    expected: &ConfiguredExposure,
) -> Result<(), Error> {
    let device = expected.plan.kind == ExposureKind::CharacterDevice;
    let exact = value.destination == expected.plan.destination
        && value.kind == expected.plan.kind
        && value.executable == expected.plan.executable
        && value.purpose == expected.plan.purpose
        && value.inventory == expected.expected
        && value.device_major == device.then_some(1)
        && value.device_minor == device.then_some(3)
        && value.mount_flags.read_only()
        && value.mount_flags.nosuid()
        && value.mount_flags.nodev() != device
        && value.mount_flags.noexec() != value.executable;
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_EXPOSURE_JOIN",
        format!("mounted exposure {} differs", value.id),
    ))
}

fn validate_namespaces(
    value: &NamespaceObservation,
    config: &Config,
) -> Result<(), Error> {
    let user_map = format!("0 {} 1", config.outside_uid);
    let group_map = format!("0 {} 1", config.outside_gid);
    if value.complete()
        && value.uid_map == user_map
        && value.gid_map == group_map
    {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_NAMESPACE_EVIDENCE",
        "retained namespace evidence is incomplete",
    ))
}

fn validate_scratch(
    result: &SetupResult,
    config: &Config,
) -> Result<(), Error> {
    let value = &result.pid1.scratch;
    let exact = value.size_limit_bytes == config.scratch_limits.size_bytes
        && value.inode_limit == config.scratch_limits.inode_count
        && value.allocated_bytes <= value.size_limit_bytes
        && value.inode_count <= value.inode_limit
        && value.logical_bytes <= config.resource_limits.output_file_bytes
        && value.mode_owner_only
        && !value.mount_flags.read_only()
        && value.mount_flags.nosuid()
        && value.mount_flags.nodev()
        && value.mount_flags.noexec();
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_SCRATCH_EVIDENCE",
        "retained scratch evidence differs from its bounds",
    ))
}

fn evidence(
    plan: &Plan,
    setup: &SetupResult,
    process: &ProcessResult,
) -> Result<Evidence, Error> {
    if process_namespace_present(process.leader_pid) {
        return Err(Error::new(
            "PREPARED_NAMESPACE_RESIDUE",
            "setup process namespace remains visible after reap",
        ));
    }
    Ok(Evidence {
        execution_environment: plan.execution_environment.clone(),
        exposures: setup.exposures.clone(),
        namespaces: setup.pid1.namespaces.clone(),
        network_before: setup.pid1.network_before.clone(),
        network_after: setup.pid1.network_after.clone(),
        scratch: setup.pid1.scratch.clone(),
        workload: setup.pid1.workload.clone(),
        cleanup: CleanupObservation::verified(),
    })
}

fn process_namespace_present(pid: u32) -> bool {
    let path = PathBuf::from(format!("/proc/{pid}/ns"));
    path.exists()
}

#[cfg(test)]
pub(super) fn validate_namespaces_for_test(
    value: &NamespaceObservation,
    config: &Config,
) -> Result<(), Error> {
    validate_namespaces(value, config)
}

#[cfg(test)]
pub(super) fn validate_process_for_test(
    value: &ProcessResult,
    config: &Config,
) -> Result<(), Error> {
    validate_process(value, config)
}
