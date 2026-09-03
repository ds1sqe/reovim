//! Effectful Linux prepared-root execution for the fixed Product proof.

mod model;

#[cfg(target_os = "linux")]
mod config_transport;
#[cfg(target_os = "linux")]
mod diagnostic;
#[cfg(target_os = "linux")]
mod inventory;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod network;
#[cfg(target_os = "linux")]
mod observation;
#[cfg(target_os = "linux")]
mod parent;
#[cfg(target_os = "linux")]
mod pid1;
#[cfg(target_os = "linux")]
mod protocol;
#[cfg(target_os = "linux")]
mod reaper;
#[cfg(target_os = "linux")]
mod setup;

#[cfg(test)]
mod tests;

use serde_json::Value;

pub use model::{
    Capability, DispatchBounds, Error, Evidence, Execution, ExposureKind,
    ExposureObservation, ExposurePlan, InventoryEntry, InventoryEntryKind,
    InventoryIdentity, InventoryLimits, Plan, ResourceLimits, ScratchLimits,
    WorkloadContext,
};
pub use model::{PID1_ARGUMENT, SETUP_ARGUMENT};

#[cfg(test)]
pub use model::ConfigTransportInspection;

pub fn validate_inventory_identity(
    identity: &InventoryIdentity,
) -> Result<(), Error> {
    inventory::validate_identity(identity)
}

#[cfg(test)]
pub fn inventory_identity(
    entries: Vec<InventoryEntry>,
) -> Result<InventoryIdentity, Error> {
    inventory::identity_from_entries(entries)
}

/// Executes one parent-supervised prepared-root journey.
///
/// # Errors
///
/// Returns a typed preflight, setup, workload, observation, or cleanup error.
#[cfg(target_os = "linux")]
pub fn execute(plan: &Plan) -> Result<Execution, Error> {
    parent::execute(plan)
}

/// Measures the exact config transport without starting prepared execution.
///
/// This may inventory configured exposures and briefly hold an empty
/// mountpoint directory. It creates no namespace, mount, or child process and
/// reconciles the temporary directory before returning.
///
/// # Errors
///
/// Returns a typed plan, inventory, encoding, transport, identity, or cleanup
/// error.
#[cfg(all(test, target_os = "linux"))]
pub fn inspect_config_transport(
    plan: &Plan,
) -> Result<ConfigTransportInspection, Error> {
    parent::inspect_config_transport(plan)
}

/// Handles the fixed outer same-binary setup dispatch.
///
/// # Errors
///
/// Returns a typed framing, setup, observation, or cleanup error.
#[cfg(target_os = "linux")]
pub fn setup_entry(bounds: DispatchBounds) -> Result<(), Error> {
    use std::io::Write as _;
    use std::os::unix::ffi::OsStringExt as _;

    require_setup_environment()?;
    require_setup_arguments()?;
    let value =
        std::env::var_os(model::CONFIG_ENVIRONMENT).ok_or_else(|| {
            Error::new(
                "PREPARED_CONFIG_ENV_MISSING",
                "setup config environment slot is absent",
            )
        })?;
    let bytes = value.into_vec();
    config_transport::inspect_length(bytes.len(), bounds.config_bytes)?;
    let config = protocol::parse_config(&bytes, bounds)?;
    let result = setup::run(&config)?;
    let mut output = protocol::encode_setup_result(&result, bounds)?;
    output.push(b'\n');
    if output.len() > bounds.result_bytes {
        return Err(Error::new(
            "PREPARED_SETUP_RESULT_LIMIT",
            "setup result plus framing exceeds its byte limit",
        ));
    }
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(&output).map_err(|failure| {
        Error::new("PREPARED_SETUP_RESULT_WRITE", failure.to_string())
    })?;
    stdout.flush().map_err(|failure| {
        Error::new("PREPARED_SETUP_RESULT_FLUSH", failure.to_string())
    })
}

/// Handles namespace PID 1 with the one compiled first-party workload.
///
/// # Errors
///
/// Returns a typed dispatch, isolation, workload, or observation error.
#[cfg(target_os = "linux")]
pub fn pid1_entry(
    bounds: DispatchBounds,
    config_json: &[u8],
    workload: fn(&WorkloadContext) -> Result<Value, Error>,
) -> Result<(), Error> {
    require_pid1_environment()?;
    require_pid1_arguments(config_json)?;
    config_transport::inspect_length(config_json.len(), bounds.config_bytes)?;
    let config = protocol::parse_config(config_json, bounds)?;
    pid1::run(&config, workload)
}

/// Reports statically detectable prepared-root capability failures.
#[cfg(target_os = "linux")]
#[must_use]
pub fn capability_probe() -> Capability {
    use std::os::unix::fs::FileTypeExt as _;

    if !linux::namespace_limit_available() {
        return Capability::Unavailable {
            code: "PREPARED_NAMESPACE_DISABLED",
            message: "unprivileged user namespaces are disabled".to_owned(),
        };
    }
    for path in ["/proc/self/ns/user", "/proc/self/mountinfo"] {
        if !std::path::Path::new(path).exists() {
            return Capability::Unavailable {
                code: "PREPARED_PROC_UNAVAILABLE",
                message: format!("required procfs path {path} is absent"),
            };
        }
    }
    let null = std::fs::symlink_metadata(linux::NULL_PATH);
    if !null.is_ok_and(|value| value.file_type().is_char_device()) {
        return Capability::Unavailable {
            code: "PREPARED_NULL_UNAVAILABLE",
            message: "the exact /dev/null device is unavailable".to_owned(),
        };
    }
    Capability::Ready
}

#[cfg(target_os = "linux")]
fn require_setup_environment() -> Result<(), Error> {
    let expected = [model::CONFIG_ENVIRONMENT, "TERM"];
    require_environment(&expected)
}

#[cfg(target_os = "linux")]
fn require_pid1_environment() -> Result<(), Error> {
    require_environment(&["TERM"])
}

#[cfg(target_os = "linux")]
fn require_environment(expected: &[&str]) -> Result<(), Error> {
    let mut actual =
        std::env::vars_os().map(|(name, _)| name).collect::<Vec<_>>();
    actual.sort();
    let mut expected =
        expected.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    expected.sort();
    let term = std::env::var_os("TERM");
    if actual == expected
        && term.as_deref() == Some(std::ffi::OsStr::new("dumb"))
    {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_ENVIRONMENT",
        "hidden dispatch environment is not exact and closed",
    ))
}

#[cfg(target_os = "linux")]
fn require_setup_arguments() -> Result<(), Error> {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    let expected = [
        std::env::current_exe()
            .map_err(|failure| {
                Error::new("PREPARED_ARGV_EXECUTABLE", failure.to_string())
            })?
            .into_os_string(),
        std::ffi::OsString::from(SETUP_ARGUMENT),
    ];
    if arguments == expected {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_SETUP_ARGV",
        "setup argv is not the exact hidden dispatch",
    ))
}

#[cfg(target_os = "linux")]
fn require_pid1_arguments(config_json: &[u8]) -> Result<(), Error> {
    use std::os::unix::ffi::OsStringExt as _;

    let arguments = std::env::args_os().collect::<Vec<_>>();
    let expected = [
        std::env::current_exe()
            .map_err(|failure| {
                Error::new("PREPARED_ARGV_EXECUTABLE", failure.to_string())
            })?
            .into_os_string(),
        std::ffi::OsString::from(PID1_ARGUMENT),
        std::ffi::OsString::from_vec(config_json.to_vec()),
    ];
    if arguments == expected {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_PID1_ARGV",
        "PID1 argv is not the exact hidden dispatch",
    ))
}

/// Non-Linux targets cannot make the Linux prepared-root claim.
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn capability_probe() -> Capability {
    Capability::Unavailable {
        code: "PREPARED_PLATFORM_UNAVAILABLE",
        message: "prepared-root execution requires Linux".to_owned(),
    }
}

#[cfg(all(test, not(target_os = "linux")))]
/// Returns the typed non-Linux platform failure.
///
/// # Errors
///
/// Always returns `PREPARED_PLATFORM_UNAVAILABLE`.
pub fn execute(_plan: &Plan) -> Result<Execution, Error> {
    Err(Error::new(
        "PREPARED_PLATFORM_UNAVAILABLE",
        "prepared-root execution requires Linux",
    ))
}

#[cfg(all(test, not(target_os = "linux")))]
/// Returns the typed non-Linux platform failure.
///
/// # Errors
///
/// Always returns `PREPARED_PLATFORM_UNAVAILABLE`.
pub fn inspect_config_transport(
    _plan: &Plan,
) -> Result<ConfigTransportInspection, Error> {
    Err(Error::new(
        "PREPARED_PLATFORM_UNAVAILABLE",
        "prepared-root execution requires Linux",
    ))
}

#[cfg(not(target_os = "linux"))]
/// Returns the typed non-Linux platform failure.
///
/// # Errors
///
/// Always returns `PREPARED_PLATFORM_UNAVAILABLE`.
pub fn setup_entry(_bounds: DispatchBounds) -> Result<(), Error> {
    Err(Error::new(
        "PREPARED_PLATFORM_UNAVAILABLE",
        "prepared-root execution requires Linux",
    ))
}

#[cfg(not(target_os = "linux"))]
/// Returns the typed non-Linux platform failure.
///
/// # Errors
///
/// Always returns `PREPARED_PLATFORM_UNAVAILABLE`.
pub fn pid1_entry(
    _bounds: DispatchBounds,
    _config_json: &[u8],
    _workload: fn(&WorkloadContext) -> Result<Value, Error>,
) -> Result<(), Error> {
    Err(Error::new(
        "PREPARED_PLATFORM_UNAVAILABLE",
        "prepared-root execution requires Linux",
    ))
}
