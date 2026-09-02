//! Shared effectful Linux prepared-root mechanisms.

use std::{
    collections::BTreeSet,
    fs::{self, DirBuilder, File, Metadata, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::fs::{
        DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _,
        PermissionsExt as _,
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use nix::{
    fcntl::{OFlag, open},
    mount::{MntFlags, MsFlags, mount, umount2},
    sys::stat::Mode,
    sys::statvfs::{FsFlags, statvfs},
};

use super::{
    inventory,
    model::{
        Config, ConfiguredExposure, DispatchBounds, Error, ExposureKind,
        ExposureObservation, MountFact, MountFlags, NamespaceLinks, Plan,
    },
};

pub(super) const CONFIG_ARG_BYTES: usize = 64 * 1024;
pub(super) const SCRATCH_PATH: &str = "/scratch";
pub(super) const PROC_PATH: &str = "/proc";
pub(super) const NULL_PATH: &str = "/dev/null";
pub(super) const MOUNT_MODE: u32 = 0o700;
const TMPFS_TYPE: &str = "tmpfs";

static NEXT_MOUNTPOINT: AtomicU64 = AtomicU64::new(0);

fn canonical_directory(path: &Path, subject: &str) -> Result<PathBuf, Error> {
    canonical_kind(path, subject, true)
}

fn canonical_file(path: &Path, subject: &str) -> Result<PathBuf, Error> {
    let value = canonical_kind(path, subject, false)?;
    let metadata = fs::symlink_metadata(&value).map_err(|failure| {
        Error::at("PREPARED_CANONICAL_METADATA", subject, failure.to_string())
    })?;
    if metadata.permissions().mode() & 0o111 != 0 {
        return Ok(value);
    }
    Err(Error::at(
        "PREPARED_CHECKER_EXECUTABLE",
        subject,
        "checker file is not executable",
    ))
}

fn canonical_kind(
    path: &Path,
    subject: &str,
    directory: bool,
) -> Result<PathBuf, Error> {
    if !path.is_absolute() {
        return Err(Error::at(
            "PREPARED_CANONICAL_PATH",
            subject,
            "path is not absolute",
        ));
    }
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at("PREPARED_CANONICAL_METADATA", subject, failure.to_string())
    })?;
    let kind = if directory { metadata.is_dir() } else { metadata.is_file() };
    if metadata.file_type().is_symlink() || !kind {
        return Err(Error::at(
            "PREPARED_CANONICAL_KIND",
            subject,
            "path has the wrong type or is a symlink",
        ));
    }
    fs::canonicalize(path).map_err(|failure| {
        Error::at("PREPARED_CANONICAL_PATH", subject, failure.to_string())
    })
}

pub(super) fn validate_plan(plan: &Plan) -> Result<(), Error> {
    validate_positive_limits(plan)?;
    validate_id(&plan.execution_environment, "execution environment")?;
    validate_workload_request(plan)?;
    let repository = canonical_directory(&plan.repository_root, "repository")?;
    let parent = canonical_directory(&plan.mount_parent, "mount parent")?;
    if repository != plan.repository_root || parent != plan.mount_parent {
        return Err(Error::new(
            "PREPARED_ROOT_ALIAS",
            "repository and mount parent must use canonical paths",
        ));
    }
    if repository.starts_with(&parent) || parent.starts_with(&repository) {
        return Err(Error::new(
            "PREPARED_MOUNT_PARENT_SCOPE",
            "mount parent and repository must be disjoint",
        ));
    }
    let checker = canonical_file(&plan.checker_executable, "checker")?;
    if checker != plan.checker_executable {
        return Err(Error::new(
            "PREPARED_CHECKER_ALIAS",
            "checker path differs from its canonical identity",
        ));
    }
    validate_exposures(plan)
}

fn validate_workload_request(plan: &Plan) -> Result<(), Error> {
    let bytes =
        serde_json::to_vec(&plan.workload_request).map_err(|failure| {
            Error::new("PREPARED_REQUEST_ENCODE", failure.to_string())
        })?;
    if bytes.len() > plan.dispatch_bounds.config_bytes {
        return Err(Error::new(
            "PREPARED_REQUEST_BYTE_LIMIT",
            "workload request exceeds the config byte limit",
        ));
    }
    let parsed = crate::json::parse(
        &bytes,
        plan.dispatch_bounds.json_depth,
        plan.dispatch_bounds.json_items,
    )
    .map_err(|failure| {
        Error::new("PREPARED_REQUEST_JSON", failure.to_string())
    })?;
    if parsed == plan.workload_request {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_REQUEST_ROUNDTRIP",
        "workload request changes across canonical JSON",
    ))
}

pub(super) fn validate_config(config: &Config) -> Result<(), Error> {
    let parent = config.mountpoint.parent().ok_or_else(|| {
        Error::new(
            "PREPARED_MOUNTPOINT_PARENT",
            "mountpoint does not have a parent",
        )
    })?;
    let plan = Plan {
        repository_root: config.repository_root.clone(),
        mount_parent: parent.to_path_buf(),
        checker_executable: config.checker_executable.clone(),
        execution_environment: config.execution_environment.clone(),
        workload_request: config.workload_request.clone(),
        exposures: config
            .exposures
            .iter()
            .map(|value| value.plan.clone())
            .collect(),
        inventory_limits: config.inventory_limits,
        scratch_limits: config.scratch_limits,
        resource_limits: config.resource_limits,
        dispatch_bounds: config.dispatch_bounds,
        workload_result_bytes: config.workload_result_bytes,
        descendant_reap_limit: config.descendant_reap_limit,
    };
    validate_plan(&plan)?;
    validate_config_mountpoint(config)?;
    if namespace_links()? != config.parent_namespaces {
        return Err(Error::new(
            "PREPARED_PARENT_NAMESPACE_DRIFT",
            "setup process inherited different namespace identities",
        ));
    }
    require_configured_identities(config)
}

fn validate_config_mountpoint(config: &Config) -> Result<(), Error> {
    let canonical =
        fs::canonicalize(&config.mountpoint).map_err(|failure| {
            Error::new("PREPARED_MOUNTPOINT_CANONICAL", failure.to_string())
        })?;
    let metadata =
        fs::symlink_metadata(&config.mountpoint).map_err(|failure| {
            Error::new("PREPARED_MOUNTPOINT_METADATA", failure.to_string())
        })?;
    let (uid, gid) = current_ids()?;
    let exact = canonical == config.mountpoint
        && metadata.is_dir()
        && metadata.uid() == uid
        && metadata.gid() == gid
        && uid == config.outside_uid
        && gid == config.outside_gid
        && metadata.permissions().mode() & 0o777 == MOUNT_MODE
        && directory_empty(&config.mountpoint)?
        && !mount_present(&config.mountpoint)?;
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_MOUNTPOINT_IDENTITY",
        "setup mountpoint identity differs from parent config",
    ))
}

fn validate_positive_limits(plan: &Plan) -> Result<(), Error> {
    let dispatch = plan.dispatch_bounds;
    let valid = dispatch.config_bytes > 0
        && dispatch.result_bytes > 0
        && dispatch.json_depth > 0
        && dispatch.json_items > 0
        && !dispatch.execution_deadline.is_zero()
        && dispatch.supervision_deadline > dispatch.execution_deadline
        && plan.scratch_limits.size_bytes > 0
        && plan.scratch_limits.inode_count > 0
        && plan.inventory_limits.path_bytes > 0
        && plan.inventory_limits.entry_count > 0
        && plan.inventory_limits.total_bytes > 0
        && plan.resource_limits.address_space_bytes > 0
        && plan.resource_limits.cpu_seconds > 0
        && plan.resource_limits.open_files > 2
        && plan.resource_limits.output_file_bytes > 0
        && plan.workload_result_bytes > 0
        && plan.descendant_reap_limit > 0;
    if valid {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_LIMITS",
        "prepared-root limits are zero or internally inconsistent",
    ))
}

fn validate_exposures(plan: &Plan) -> Result<(), Error> {
    if plan.exposures.is_empty()
        || plan.exposures.len() > plan.inventory_limits.entry_count
    {
        return Err(Error::new(
            "PREPARED_EXPOSURE_COUNT",
            "exposure count is empty or exceeds its bound",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    let mut null_devices = 0_usize;
    let mut checker_exposures = 0_usize;
    let mut repository_files = 0_usize;
    for exposure in &plan.exposures {
        validate_exposure(exposure, plan, &mut null_devices)?;
        checker_exposures += usize::from(is_checker_exposure(exposure, plan));
        repository_files += usize::from(is_repository_file(exposure, plan)?);
        if !ids.insert(exposure.id.clone())
            || !destinations.insert(exposure.destination.clone())
        {
            return Err(Error::new(
                "PREPARED_EXPOSURE_DUPLICATE",
                "exposure ID or destination is duplicated",
            ));
        }
    }
    reject_destination_overlap(&destinations)?;
    if null_devices == 1 && checker_exposures == 1 && repository_files > 0 {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_REQUIRED_EXPOSURE",
        "one null, one checker, and repository file exposures are required",
    ))
}

fn is_checker_exposure(
    exposure: &super::model::ExposurePlan,
    plan: &Plan,
) -> bool {
    exposure.source == plan.checker_executable
        && exposure.kind == ExposureKind::File
        && exposure.executable
}

fn is_repository_file(
    exposure: &super::model::ExposurePlan,
    plan: &Plan,
) -> Result<bool, Error> {
    let repository_purpose = exposure.purpose == "Product repository input";
    let repository_destination = exposure.destination.starts_with("/repo");
    if !repository_purpose && !repository_destination {
        if exposure.source != plan.repository_root {
            return Ok(false);
        }
        return Err(Error::new(
            "PREPARED_REPOSITORY_EXPOSURE",
            "whole-repository exposure is forbidden",
        ));
    }
    let exact = exposure.source.starts_with(&plan.repository_root)
        && exposure.source != plan.repository_root
        && exposure.kind == ExposureKind::File
        && !exposure.executable
        && exposure.destination.starts_with("/repo")
        && exposure.destination != Path::new("/repo")
        && repository_purpose;
    if exact {
        return Ok(true);
    }
    Err(Error::new(
        "PREPARED_REPOSITORY_EXPOSURE",
        "repository authority is not an exact non-executable file exposure",
    ))
}

fn validate_exposure(
    exposure: &super::model::ExposurePlan,
    plan: &Plan,
    null_devices: &mut usize,
) -> Result<(), Error> {
    validate_label(&exposure.id, "exposure ID", 1024)?;
    validate_label(&exposure.purpose, "exposure purpose", 256)?;
    validate_destination(
        &exposure.destination,
        plan.inventory_limits.path_bytes,
    )?;
    let source = fs::canonicalize(&exposure.source).map_err(|failure| {
        Error::at(
            "PREPARED_EXPOSURE_SOURCE",
            exposure.source.display().to_string(),
            failure.to_string(),
        )
    })?;
    if source != exposure.source || source.starts_with(&plan.mount_parent) {
        return Err(Error::new(
            "PREPARED_EXPOSURE_SOURCE_ALIAS",
            "exposure source is aliased or beneath the mount parent",
        ));
    }
    if exposure.kind == ExposureKind::CharacterDevice {
        validate_null_exposure(exposure)?;
        *null_devices = null_devices.saturating_add(1);
    }
    Ok(())
}

fn validate_null_exposure(
    exposure: &super::model::ExposurePlan,
) -> Result<(), Error> {
    let exact = exposure.source == Path::new(NULL_PATH)
        && exposure.destination == Path::new(NULL_PATH)
        && !exposure.executable;
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_DEVICE_IDENTITY",
        "character-device exposure is not the exact /dev/null mapping",
    ))
}

fn validate_label(
    value: &str,
    subject: &str,
    limit: usize,
) -> Result<(), Error> {
    let valid = !value.is_empty()
        && value.len() <= limit
        && !value.contains(['\0', '\r', '\n']);
    if valid {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_LABEL",
        format!("{subject} is empty, unframed, or exceeds {limit} bytes"),
    ))
}

fn validate_id(value: &str, subject: &str) -> Result<(), Error> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_')
        });
    if valid {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_IDENTIFIER",
        format!("{subject} is not normalized lower ASCII"),
    ))
}

fn validate_destination(path: &Path, limit: usize) -> Result<(), Error> {
    let text = path.to_str().ok_or_else(|| {
        Error::new("PREPARED_DESTINATION_UTF8", "destination is not UTF-8")
    })?;
    let valid = text.starts_with('/')
        && text != "/"
        && !text.ends_with('/')
        && !text.contains(['\0', '\r', '\n', '\\'])
        && !text.contains("//")
        && !text.split('/').any(|part| matches!(part, "." | ".."))
        && text.len() <= limit;
    let reserved = path == Path::new(PROC_PATH)
        || path.starts_with(format!("{PROC_PATH}/"))
        || path == Path::new(SCRATCH_PATH)
        || path.starts_with(format!("{SCRATCH_PATH}/"));
    if valid && !reserved {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_DESTINATION",
        format!("invalid or reserved exposure destination {text:?}"),
    ))
}

fn reject_destination_overlap(paths: &BTreeSet<PathBuf>) -> Result<(), Error> {
    for left in paths {
        if paths.iter().any(|right| left != right && right.starts_with(left)) {
            return Err(Error::new(
                "PREPARED_EXPOSURE_OVERLAP",
                format!("exposure destination {} overlaps", left.display()),
            ));
        }
    }
    Ok(())
}

pub(super) fn configure_exposures(
    plan: &Plan,
) -> Result<Vec<ConfiguredExposure>, Error> {
    let mut values = plan
        .exposures
        .iter()
        .map(|exposure| {
            let expected = inventory::observe(
                &exposure.source,
                exposure.kind,
                plan.inventory_limits,
            )?;
            Ok(ConfiguredExposure { plan: exposure.clone(), expected })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    values.sort_by(|left, right| left.plan.id.cmp(&right.plan.id));
    Ok(values)
}

pub(super) fn require_configured_identities(
    config: &Config,
) -> Result<(), Error> {
    for exposure in &config.exposures {
        let observed = inventory::observe(
            &exposure.plan.source,
            exposure.plan.kind,
            config.inventory_limits,
        )?;
        if observed != exposure.expected {
            return Err(Error::new(
                "PREPARED_SOURCE_DRIFT",
                format!("exposure {} changed", exposure.plan.id),
            ));
        }
    }
    Ok(())
}

pub(super) fn prepared_path(
    root: &Path,
    absolute: &Path,
) -> Result<PathBuf, Error> {
    let relative = absolute
        .strip_prefix("/")
        .map_err(|failure| Error::new("PREPARED_PATH", failure.to_string()))?;
    Ok(root.join(relative))
}

pub(super) fn mount_scratch(
    config: &Config,
    mounts: &mut MountStack,
) -> Result<(), Error> {
    let data = format!(
        "mode=0700,size={},nr_inodes={}",
        config.scratch_limits.size_bytes, config.scratch_limits.inode_count
    );
    let flags = MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC;
    mount(
        Some(TMPFS_TYPE),
        &config.mountpoint,
        Some(TMPFS_TYPE),
        flags,
        Some(data.as_str()),
    )
    .map_err(|failure| {
        Error::new("PREPARED_TMPFS_MOUNT", failure.to_string())
    })?;
    mounts.push(config.mountpoint.clone(), MountKind::Scratch);
    verify_tmpfs(config)?;
    for path in [SCRATCH_PATH, PROC_PATH, "/dev"] {
        fs::create_dir_all(prepared_path(&config.mountpoint, Path::new(path))?)
            .map_err(|failure| {
                Error::at("PREPARED_ROOT_CREATE", path, failure.to_string())
            })?;
    }
    fs::set_permissions(
        prepared_path(&config.mountpoint, Path::new(SCRATCH_PATH))?,
        fs::Permissions::from_mode(MOUNT_MODE),
    )
    .map_err(|failure| {
        Error::new("PREPARED_SCRATCH_MODE", failure.to_string())
    })?;
    Ok(())
}

fn verify_tmpfs(config: &Config) -> Result<(), Error> {
    let metadata = fs::metadata(&config.mountpoint).map_err(|failure| {
        Error::new("PREPARED_TMPFS_METADATA", failure.to_string())
    })?;
    let mode = metadata.permissions().mode() & 0o777;
    let stats = statvfs(&config.mountpoint).map_err(|failure| {
        Error::new("PREPARED_TMPFS_STATVFS", failure.to_string())
    })?;
    let capacity =
        stats.blocks().checked_mul(stats.fragment_size()).ok_or_else(|| {
            Error::new("PREPARED_TMPFS_CAPACITY", "tmpfs capacity overflow")
        })?;
    let flags = stats.flags();
    let exact = mode == MOUNT_MODE
        && capacity == config.scratch_limits.size_bytes
        && stats.files() == config.scratch_limits.inode_count
        && flags.contains(FsFlags::ST_NOSUID)
        && flags.contains(FsFlags::ST_NODEV)
        && flags.contains(FsFlags::ST_NOEXEC);
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_TMPFS_IDENTITY",
        "tmpfs quota, mode, or flags differ from the request",
    ))
}

pub(super) fn mount_exposures(
    config: &Config,
    mounts: &mut MountStack,
) -> Result<Vec<ExposureObservation>, Error> {
    let mut observations = Vec::new();
    for exposure in &config.exposures {
        reject_nested_mounts(&exposure.plan.source, config.dispatch_bounds)?;
        let destination =
            prepared_path(&config.mountpoint, &exposure.plan.destination)?;
        create_mount_target(&destination, exposure.plan.kind)?;
        mount_bind(exposure, &destination, mounts)?;
        observations.push(observe_mounted_exposure(
            exposure,
            &destination,
            config,
        )?);
    }
    observations.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(observations)
}

fn create_mount_target(path: &Path, kind: ExposureKind) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|failure| {
            Error::at(
                "PREPARED_EXPOSURE_TARGET",
                path.display().to_string(),
                failure.to_string(),
            )
        })?;
    }
    match kind {
        ExposureKind::Directory => fs::create_dir(path),
        ExposureKind::File | ExposureKind::CharacterDevice => {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map(|_| ())
        }
    }
    .map_err(|failure| {
        Error::at(
            "PREPARED_EXPOSURE_TARGET",
            path.display().to_string(),
            failure.to_string(),
        )
    })
}

fn mount_bind(
    exposure: &ConfiguredExposure,
    destination: &Path,
    mounts: &mut MountStack,
) -> Result<(), Error> {
    mount(
        Some(&exposure.plan.source),
        destination,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    )
    .map_err(|failure| {
        Error::new("PREPARED_BIND_MOUNT", failure.to_string())
    })?;
    mounts.push(destination.to_path_buf(), MountKind::Exposure);
    let mut flags = MsFlags::MS_BIND
        | MsFlags::MS_REMOUNT
        | MsFlags::MS_RDONLY
        | MsFlags::MS_NOSUID;
    if exposure.plan.kind != ExposureKind::CharacterDevice {
        flags |= MsFlags::MS_NODEV;
    }
    if !exposure.plan.executable {
        flags |= MsFlags::MS_NOEXEC;
    }
    mount(None::<&str>, destination, None::<&str>, flags, None::<&str>).map_err(
        |failure| Error::new("PREPARED_BIND_REMOUNT", failure.to_string()),
    )
}

pub(super) fn observe_mounted_exposure(
    exposure: &ConfiguredExposure,
    destination: &Path,
    config: &Config,
) -> Result<ExposureObservation, Error> {
    let observed = inventory::observe(
        destination,
        exposure.plan.kind,
        config.inventory_limits,
    )?;
    if observed != exposure.expected {
        return Err(Error::new(
            "PREPARED_EXPOSURE_INVENTORY",
            format!("mounted exposure {} changed", exposure.plan.id),
        ));
    }
    let stats = statvfs(destination).map_err(|failure| {
        Error::new("PREPARED_EXPOSURE_STATVFS", failure.to_string())
    })?;
    let flags = stats.flags();
    let device = exposure.plan.kind == ExposureKind::CharacterDevice;
    let noexec = !exposure.plan.executable;
    let exact = flags.contains(FsFlags::ST_RDONLY)
        && flags.contains(FsFlags::ST_NOSUID)
        && flags.contains(FsFlags::ST_NODEV) != device
        && flags.contains(FsFlags::ST_NOEXEC) == noexec;
    if !exact {
        return Err(Error::new(
            "PREPARED_EXPOSURE_FLAGS",
            format!("exposure {} has wrong flags", exposure.plan.id),
        ));
    }
    let mut mount_flags = MountFlags::empty();
    mount_flags.record(MountFact::ReadOnly, true);
    mount_flags.record(MountFact::Nosuid, true);
    mount_flags.record(MountFact::Nodev, !device);
    mount_flags.record(MountFact::Noexec, noexec);
    Ok(ExposureObservation {
        id: exposure.plan.id.clone(),
        destination: exposure.plan.destination.clone(),
        kind: exposure.plan.kind,
        executable: exposure.plan.executable,
        purpose: exposure.plan.purpose.clone(),
        inventory: observed,
        device_major: device.then_some(1),
        device_minor: device.then_some(3),
        mount_flags,
    })
}

fn reject_nested_mounts(
    path: &Path,
    bounds: DispatchBounds,
) -> Result<(), Error> {
    let contents = read_bounded("/proc/self/mountinfo", bounds.result_bytes)?;
    let text = std::str::from_utf8(&contents).map_err(|failure| {
        Error::new("PREPARED_MOUNTINFO_UTF8", failure.to_string())
    })?;
    for line in text.lines() {
        let Some(mountpoint) = line.split_ascii_whitespace().nth(4) else {
            return Err(Error::new(
                "PREPARED_MOUNTINFO_FORMAT",
                "mountinfo row lacks a mountpoint",
            ));
        };
        if mountpoint.contains('\\') {
            return Err(Error::new(
                "PREPARED_MOUNTINFO_ESCAPE",
                "escaped mountinfo paths are outside this parser profile",
            ));
        }
        let mountpoint = Path::new(mountpoint);
        if mountpoint != path && mountpoint.starts_with(path) {
            return Err(Error::new(
                "PREPARED_EXPOSURE_NESTED_MOUNT",
                format!("{} contains {}", path.display(), mountpoint.display()),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MountKind {
    Scratch,
    Exposure,
    Proc,
}

#[derive(Debug)]
struct MountedPath {
    path: PathBuf,
    kind: MountKind,
}

#[derive(Debug, Default)]
pub(super) struct MountStack {
    paths: Vec<MountedPath>,
}

impl MountStack {
    pub(super) fn push(&mut self, path: PathBuf, kind: MountKind) {
        self.paths.push(MountedPath { path, kind });
    }

    pub(super) fn cleanup(&mut self) -> MountCleanup {
        self.register_possible_proc_mount();
        let mut cleanup = MountCleanup::default();
        let mut failure = None;
        let mut retained = Vec::new();
        while let Some(value) = self.paths.pop() {
            retain_failed_unmount(
                value,
                &mut cleanup,
                &mut failure,
                &mut retained,
            );
        }
        retained.reverse();
        self.paths = retained;
        cleanup.result = failure.map_or(Ok(()), Err);
        cleanup
    }

    fn register_possible_proc_mount(&mut self) {
        if self.paths.iter().any(|value| value.kind == MountKind::Proc) {
            return;
        }
        let Some(path) = self
            .paths
            .iter()
            .find(|value| value.kind == MountKind::Scratch)
            .map(|value| value.path.join("proc"))
        else {
            return;
        };
        self.paths.push(MountedPath { path, kind: MountKind::Proc });
    }

    #[cfg(test)]
    pub(super) const fn pending_mounts_for_test(&self) -> usize {
        self.paths.len()
    }
}

impl Drop for MountStack {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[derive(Debug)]
pub(super) struct MountCleanup {
    pub proc_unmounted: bool,
    pub exposures_unmounted: bool,
    pub scratch_unmounted: bool,
    pub result: Result<(), Error>,
}

impl Default for MountCleanup {
    fn default() -> Self {
        Self {
            proc_unmounted: false,
            exposures_unmounted: true,
            scratch_unmounted: false,
            result: Ok(()),
        }
    }
}

fn record_unmount(
    path: &Path,
    kind: MountKind,
    cleanup: &mut MountCleanup,
    failure: &mut Option<Error>,
) -> bool {
    match umount2(path, MntFlags::empty()) {
        Ok(()) => {
            match kind {
                MountKind::Proc => cleanup.proc_unmounted = true,
                MountKind::Exposure => {}
                MountKind::Scratch => cleanup.scratch_unmounted = true,
            }
            true
        }
        Err(error)
            if kind == MountKind::Proc
                && matches!(
                    error,
                    nix::errno::Errno::EINVAL | nix::errno::Errno::ENOENT
                ) =>
        {
            cleanup.proc_unmounted = true;
            true
        }
        Err(error) => {
            if kind == MountKind::Exposure {
                cleanup.exposures_unmounted = false;
            }
            if failure.is_none() {
                *failure = Some(Error::at(
                    "PREPARED_UNMOUNT",
                    path.display().to_string(),
                    error.to_string(),
                ));
            }
            false
        }
    }
}

fn retain_failed_unmount(
    value: MountedPath,
    cleanup: &mut MountCleanup,
    failure: &mut Option<Error>,
    retained: &mut Vec<MountedPath>,
) {
    if !record_unmount(&value.path, value.kind, cleanup, failure) {
        retained.push(value);
    }
}

struct CreatedMountpoint {
    path: PathBuf,
    armed: bool,
}

impl CreatedMountpoint {
    const fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for CreatedMountpoint {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let directory = fs::symlink_metadata(&self.path)
            .is_ok_and(|metadata| metadata.file_type().is_dir());
        if directory {
            let _ = fs::remove_dir(&self.path);
        }
    }
}

pub(super) struct ParentMountpoint {
    path: PathBuf,
    held: File,
    identity: FileIdentity,
    armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    owner: u32,
    mode: u32,
}

impl ParentMountpoint {
    pub(super) fn create(plan: &Plan) -> Result<Self, Error> {
        let (uid, _) = current_ids()?;
        let result = (0..128)
            .map(|_| Self::numbered_candidate(plan, uid))
            .find(|value| !matches!(value, Ok(None)))
            .transpose()?
            .flatten();
        result.ok_or_else(|| {
            Error::new(
                "PREPARED_MOUNTPOINT_COLLISION",
                "mountpoint collision retry bound exhausted",
            )
        })
    }

    fn numbered_candidate(
        plan: &Plan,
        uid: u32,
    ) -> Result<Option<Self>, Error> {
        let sequence = NEXT_MOUNTPOINT.fetch_add(1, Ordering::Relaxed);
        let path = plan.mount_parent.join(format!(
            ".reovim-prepared-{}-{sequence}",
            std::process::id()
        ));
        Self::create_candidate(path, uid)
    }

    fn create_candidate(
        path: PathBuf,
        uid: u32,
    ) -> Result<Option<Self>, Error> {
        let mut builder = DirBuilder::new();
        builder.mode(MOUNT_MODE);
        match builder.create(&path) {
            Ok(()) => {
                let created = CreatedMountpoint::new(path.clone());
                let mountpoint = Self::open(path, uid)?;
                created.disarm();
                Ok(Some(mountpoint))
            }
            Err(failure)
                if failure.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                Ok(None)
            }
            Err(failure) => Err(Error::new(
                "PREPARED_MOUNTPOINT_CREATE",
                failure.to_string(),
            )),
        }
    }

    fn open(path: PathBuf, uid: u32) -> Result<Self, Error> {
        let flags = OFlag::O_RDONLY
            | OFlag::O_DIRECTORY
            | OFlag::O_NOFOLLOW
            | OFlag::O_CLOEXEC;
        let held = open(&path, flags, Mode::empty()).map(File::from);
        let held = held.map_err(|failure| {
            Error::new("PREPARED_MOUNTPOINT_OPEN", failure.to_string())
        })?;
        held.set_permissions(fs::Permissions::from_mode(MOUNT_MODE)).map_err(
            |failure| {
                Error::new("PREPARED_MOUNTPOINT_MODE", failure.to_string())
            },
        )?;
        let metadata = held.metadata().map_err(|failure| {
            Error::new("PREPARED_MOUNTPOINT_METADATA", failure.to_string())
        })?;
        let identity = file_identity(&metadata);
        let mountpoint = Self { path, held, identity, armed: true };
        mountpoint.validate_created(uid)?;
        Ok(mountpoint)
    }

    fn validate_created(&self, uid: u32) -> Result<(), Error> {
        let named = fs::symlink_metadata(&self.path).map_err(|failure| {
            Error::new("PREPARED_MOUNTPOINT_METADATA", failure.to_string())
        })?;
        let exact = named.file_type().is_dir()
            && file_identity(&named) == self.identity
            && self.identity.owner == uid
            && self.identity.mode == MOUNT_MODE;
        if exact && directory_empty(&self.path)? {
            return Ok(());
        }
        Err(Error::new(
            "PREPARED_MOUNTPOINT_IDENTITY",
            "mountpoint is not owner-only, owned, and empty",
        ))
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn finish(mut self) -> Result<(), Error> {
        let named = fs::symlink_metadata(&self.path).map_err(|failure| {
            Error::new("PREPARED_MOUNTPOINT_RECHECK", failure.to_string())
        })?;
        let held = self.held.metadata().map_err(|failure| {
            Error::new("PREPARED_MOUNTPOINT_RECHECK", failure.to_string())
        })?;
        if !named.file_type().is_dir()
            || file_identity(&named) != self.identity
            || file_identity(&held) != self.identity
            || !directory_empty(&self.path)?
            || mount_present(&self.path)?
        {
            return Err(Error::new(
                "PREPARED_MOUNTPOINT_CLEANUP",
                "mountpoint identity, emptiness, or mount absence failed",
            ));
        }
        fs::remove_dir(&self.path).map_err(|failure| {
            Error::new("PREPARED_MOUNTPOINT_REMOVE", failure.to_string())
        })?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for ParentMountpoint {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let same = fs::symlink_metadata(&self.path).is_ok_and(|value| {
            value.file_type().is_dir() && file_identity(&value) == self.identity
        });
        let absent = mount_present(&self.path).map(|value| !value);
        if same
            && absent.unwrap_or(false)
            && directory_empty(&self.path).unwrap_or(false)
        {
            let _ = fs::remove_dir(&self.path);
        }
    }
}

fn file_identity(metadata: &Metadata) -> FileIdentity {
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        owner: metadata.uid(),
        mode: metadata.mode() & 0o777,
    }
}

fn directory_empty(path: &Path) -> Result<bool, Error> {
    let mut entries = fs::read_dir(path).map_err(|failure| {
        Error::new("PREPARED_MOUNTPOINT_READ", failure.to_string())
    })?;
    Ok(entries.next().is_none())
}

fn mount_present(path: &Path) -> Result<bool, Error> {
    let contents = read_bounded("/proc/self/mountinfo", 4 * 1024 * 1024)?;
    let text = std::str::from_utf8(&contents).map_err(|failure| {
        Error::new("PREPARED_MOUNTINFO_UTF8", failure.to_string())
    })?;
    let expected = path.to_str().ok_or_else(|| {
        Error::new("PREPARED_MOUNTPOINT_UTF8", "mountpoint is not UTF-8")
    })?;
    Ok(text
        .lines()
        .any(|line| line.split_ascii_whitespace().nth(4) == Some(expected)))
}

pub(super) fn current_ids() -> Result<(u32, u32), Error> {
    let status = read_bounded("/proc/self/status", 64 * 1024)?;
    let text = std::str::from_utf8(&status).map_err(|failure| {
        Error::new("PREPARED_STATUS_UTF8", failure.to_string())
    })?;
    let uid = first_id(status_field(text, "Uid:")?)?;
    let gid = first_id(status_field(text, "Gid:")?)?;
    Ok((uid, gid))
}

pub(super) fn status_field<'a>(
    text: &'a str,
    name: &str,
) -> Result<&'a str, Error> {
    text.lines()
        .find_map(|line| line.strip_prefix(name).map(str::trim))
        .ok_or_else(|| {
            Error::new("PREPARED_STATUS_FIELD", format!("status lacks {name}"))
        })
}

fn first_id(value: &str) -> Result<u32, Error> {
    value
        .split_ascii_whitespace()
        .next()
        .ok_or_else(|| Error::new("PREPARED_STATUS_ID", "identity is absent"))?
        .parse::<u32>()
        .map_err(|failure| {
            Error::new("PREPARED_STATUS_ID", failure.to_string())
        })
}

pub(super) fn namespace_links() -> Result<NamespaceLinks, Error> {
    Ok(NamespaceLinks {
        user: namespace_link("user")?,
        mount: namespace_link("mnt")?,
        network: namespace_link("net")?,
        pid: namespace_link("pid")?,
    })
}

fn namespace_link(name: &str) -> Result<String, Error> {
    let path = format!("/proc/self/ns/{name}");
    let value = fs::read_link(&path).map_err(|failure| {
        Error::at("PREPARED_NAMESPACE_LINK", path, failure.to_string())
    })?;
    value.into_os_string().into_string().map_err(|_| {
        Error::new(
            "PREPARED_NAMESPACE_LINK_UTF8",
            "namespace identity is not UTF-8",
        )
    })
}

pub(super) fn namespace_limit_available() -> bool {
    for path in [
        "/proc/sys/user/max_user_namespaces",
        "/proc/sys/kernel/unprivileged_userns_clone",
    ] {
        let Ok(value) = read_trimmed(path, 64) else { continue };
        if value == "0" {
            return false;
        }
    }
    true
}

pub(super) fn read_trimmed(path: &str, limit: usize) -> Result<String, Error> {
    let bytes = read_bounded(path, limit)?;
    let text = std::str::from_utf8(&bytes).map_err(|failure| {
        Error::at("PREPARED_READ_UTF8", path, failure.to_string())
    })?;
    Ok(text.trim().to_owned())
}

pub(super) fn read_bounded(path: &str, limit: usize) -> Result<Vec<u8>, Error> {
    let file = File::open(path).map_err(|failure| {
        Error::at("PREPARED_READ", path, failure.to_string())
    })?;
    let take = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    file.take(take).read_to_end(&mut bytes).map_err(|failure| {
        Error::at("PREPARED_READ", path, failure.to_string())
    })?;
    if bytes.len() <= limit {
        return Ok(bytes);
    }
    Err(Error::at(
        "PREPARED_READ_LIMIT",
        path,
        format!("input exceeds {limit} bytes"),
    ))
}

pub(super) fn write_proc(path: &str, bytes: &[u8]) -> Result<(), Error> {
    let mut file =
        OpenOptions::new().write(true).open(path).map_err(|failure| {
            Error::at("PREPARED_PROC_OPEN", path, failure.to_string())
        })?;
    file.write_all(bytes).map_err(|failure| {
        Error::at("PREPARED_PROC_WRITE", path, failure.to_string())
    })
}

pub(super) fn one_final_line<'a>(
    bytes: &'a [u8],
    code: &'static str,
) -> Result<&'a [u8], Error> {
    if bytes.is_empty()
        || bytes.last() != Some(&b'\n')
        || bytes[..bytes.len() - 1].contains(&b'\n')
        || bytes.contains(&b'\r')
        || bytes.contains(&0)
    {
        return Err(Error::new(code, "result is not one final-LF JSON line"));
    }
    Ok(&bytes[..bytes.len() - 1])
}

#[cfg(test)]
pub(super) fn validate_plan_for_test(plan: &Plan) -> Result<(), Error> {
    validate_plan(plan)
}

#[cfg(test)]
pub(super) fn create_mountpoint_candidate_for_test(
    path: PathBuf,
    uid: u32,
) -> Result<(), Error> {
    ParentMountpoint::create_candidate(path, uid).map(drop)
}

#[cfg(test)]
pub(super) fn cleanup_created_mountpoint_for_test(path: PathBuf) {
    drop(CreatedMountpoint::new(path));
}
