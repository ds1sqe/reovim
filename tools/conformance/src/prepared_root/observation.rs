//! Namespace, mount, and writable-scratch observations made by PID 1.

use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

use nix::{
    sys::statvfs::{FsFlags, statvfs},
    unistd::getpid,
};

use super::{
    linux::{
        PROC_PATH, SCRATCH_PATH, namespace_links, observe_mounted_exposure,
        read_bounded,
    },
    model::{
        Config, Error, MountFact, MountFlags, NamespaceFact,
        NamespaceObservation, ScratchObservation,
    },
    setup::identifier_maps,
};

pub(super) fn namespaces(
    config: &Config,
    parent_death_signal: bool,
) -> Result<NamespaceObservation, Error> {
    if getpid().as_raw() != 1 {
        return Err(Error::new(
            "PREPARED_PID_ONE",
            "hidden workload process is not namespace PID 1",
        ));
    }
    let (uid_map, gid_map) = identifier_maps(config)?;
    let current = namespace_links()?;
    let mut observation = NamespaceObservation::empty(uid_map, gid_map);
    observation.record(
        NamespaceFact::UserPrivate,
        current.user != config.parent_namespaces.user,
    );
    observation.record(
        NamespaceFact::MountPrivate,
        current.mount != config.parent_namespaces.mount,
    );
    observation.record(
        NamespaceFact::NetworkPrivate,
        current.network != config.parent_namespaces.network,
    );
    observation.record(
        NamespaceFact::PidPrivate,
        current.pid != config.parent_namespaces.pid,
    );
    observation.record(NamespaceFact::UidMapSingle, true);
    observation.record(NamespaceFact::GidMapSingle, true);
    observation.record(NamespaceFact::SetgroupsDenied, true);
    observation
        .record(NamespaceFact::PrivatePropagation, private_root_mount(config)?);
    observation.record(NamespaceFact::PidOne, true);
    observation.record(NamespaceFact::ParentDeathSignal, parent_death_signal);
    require_namespace_observation(&observation)?;
    Ok(observation)
}

fn require_namespace_observation(
    value: &NamespaceObservation,
) -> Result<(), Error> {
    if value.complete() {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_NAMESPACE_OBSERVATION",
        "namespace isolation observation is incomplete",
    ))
}

fn private_root_mount(config: &Config) -> Result<bool, Error> {
    let bytes = read_bounded(
        "/proc/self/mountinfo",
        config.dispatch_bounds.result_bytes,
    )?;
    let text = std::str::from_utf8(&bytes).map_err(|failure| {
        Error::new("PREPARED_MOUNTINFO_UTF8", failure.to_string())
    })?;
    for line in text.lines() {
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.get(4) == Some(&"/") {
            return private_optional_fields(&fields);
        }
    }
    Err(Error::new(
        "PREPARED_ROOT_MOUNT_MISSING",
        "mountinfo lacks the prepared root mount",
    ))
}

fn private_optional_fields(fields: &[&str]) -> Result<bool, Error> {
    let separator = fields.iter().position(|value| *value == "-");
    let Some(separator) = separator else {
        return Err(Error::new(
            "PREPARED_MOUNTINFO_FORMAT",
            "mountinfo row lacks its separator",
        ));
    };
    if separator < 6 {
        return Err(Error::new(
            "PREPARED_MOUNTINFO_FORMAT",
            "mountinfo row lacks fixed fields",
        ));
    }
    let propagated = fields[6..separator].iter().any(|value| {
        value.starts_with("shared:")
            || value.starts_with("master:")
            || value.starts_with("propagate_from:")
    });
    Ok(!propagated)
}

pub(super) fn verify_mounts(config: &Config) -> Result<(), Error> {
    let expected = expected_mounts(config);
    let observed = observed_mounts(config)?;
    if observed != expected {
        return Err(Error::new(
            "PREPARED_MOUNT_SET",
            format!("mount set differs: {observed:?}"),
        ));
    }
    for exposure in &config.exposures {
        observe_mounted_exposure(exposure, &exposure.plan.destination, config)?;
    }
    Ok(())
}

fn expected_mounts(config: &Config) -> BTreeSet<PathBuf> {
    let mut mounts =
        BTreeSet::from([PathBuf::from("/"), PathBuf::from(PROC_PATH)]);
    mounts.extend(
        config.exposures.iter().map(|value| value.plan.destination.clone()),
    );
    mounts
}

fn observed_mounts(config: &Config) -> Result<BTreeSet<PathBuf>, Error> {
    let bytes = read_bounded(
        "/proc/self/mountinfo",
        config.dispatch_bounds.result_bytes,
    )?;
    let text = std::str::from_utf8(&bytes).map_err(|failure| {
        Error::new("PREPARED_MOUNTINFO_UTF8", failure.to_string())
    })?;
    let mut mounts = BTreeSet::new();
    for line in text.lines() {
        if mounts.len() >= config.dispatch_bounds.json_items {
            return Err(Error::new(
                "PREPARED_MOUNT_COUNT_LIMIT",
                "mount count exceeds the observation limit",
            ));
        }
        let path = mountpoint(line)?;
        if !mounts.insert(path) {
            return Err(Error::new(
                "PREPARED_MOUNT_DUPLICATE",
                "mountinfo repeats a mountpoint",
            ));
        }
    }
    Ok(mounts)
}

fn mountpoint(line: &str) -> Result<PathBuf, Error> {
    let value = line.split_ascii_whitespace().nth(4).ok_or_else(|| {
        Error::new(
            "PREPARED_MOUNTINFO_FORMAT",
            "mountinfo row lacks a mountpoint",
        )
    })?;
    if !value.starts_with('/') || value.contains('\\') {
        return Err(Error::new(
            "PREPARED_MOUNTINFO_PATH",
            "mountinfo path is escaped or non-absolute",
        ));
    }
    Ok(PathBuf::from(value))
}

pub(super) fn scratch(config: &Config) -> Result<ScratchObservation, Error> {
    let stats = statvfs(SCRATCH_PATH).map_err(|failure| {
        Error::new("PREPARED_SCRATCH_STATVFS", failure.to_string())
    })?;
    let allocated_blocks = stats.blocks().checked_sub(stats.blocks_free());
    let allocated_blocks = allocated_blocks.ok_or_else(|| {
        Error::new(
            "PREPARED_SCRATCH_BLOCKS",
            "free blocks exceed scratch capacity",
        )
    })?;
    let allocated_bytes = allocated_blocks
        .checked_mul(stats.fragment_size())
        .ok_or_else(|| {
        Error::new(
            "PREPARED_SCRATCH_BLOCKS",
            "allocated scratch bytes overflow",
        )
    })?;
    let inode_count = stats.files().checked_sub(stats.files_free());
    let inode_count = inode_count.ok_or_else(|| {
        Error::new(
            "PREPARED_SCRATCH_INODES",
            "free inodes exceed scratch capacity",
        )
    })?;
    let logical_bytes = scratch_logical_bytes(config)?;
    let metadata = fs::symlink_metadata(SCRATCH_PATH).map_err(|failure| {
        Error::new("PREPARED_SCRATCH_METADATA", failure.to_string())
    })?;
    let flags = stats.flags();
    let mut mount_flags = MountFlags::empty();
    mount_flags.record(MountFact::Nosuid, flags.contains(FsFlags::ST_NOSUID));
    mount_flags.record(MountFact::Nodev, flags.contains(FsFlags::ST_NODEV));
    mount_flags.record(MountFact::Noexec, flags.contains(FsFlags::ST_NOEXEC));
    let observation = ScratchObservation {
        size_limit_bytes: config.scratch_limits.size_bytes,
        inode_limit: config.scratch_limits.inode_count,
        allocated_bytes,
        inode_count,
        logical_bytes,
        mode_owner_only: metadata.permissions().mode() & 0o777 == 0o700,
        mount_flags,
    };
    require_scratch(config, &observation)?;
    Ok(observation)
}

fn require_scratch(
    config: &Config,
    value: &ScratchObservation,
) -> Result<(), Error> {
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
        "PREPARED_SCRATCH_OBSERVATION",
        "scratch quota, use, mode, or flags differ",
    ))
}

fn scratch_logical_bytes(config: &Config) -> Result<u64, Error> {
    let mut pending = vec![PathBuf::from(SCRATCH_PATH)];
    let mut visited = 0_usize;
    let mut logical = 0_u64;
    while let Some(path) = pending.pop() {
        visited = charge_path(visited, config)?;
        let metadata = fs::symlink_metadata(&path).map_err(|failure| {
            Error::at(
                "PREPARED_SCRATCH_WALK",
                path.display().to_string(),
                failure.to_string(),
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(scratch_kind_error(&path));
        }
        if metadata.is_file() {
            logical = charge_logical(logical, metadata.len(), config)?;
            continue;
        }
        if !metadata.is_dir() {
            return Err(scratch_kind_error(&path));
        }
        push_children(&path, &mut pending, config)?;
    }
    Ok(logical)
}

fn charge_path(current: usize, config: &Config) -> Result<usize, Error> {
    let next = current.checked_add(1).ok_or_else(|| {
        Error::new("PREPARED_SCRATCH_PATH_OVERFLOW", "path count overflow")
    })?;
    if next <= config.inventory_limits.entry_count {
        return Ok(next);
    }
    Err(Error::new(
        "PREPARED_SCRATCH_PATH_LIMIT",
        "scratch path count exceeds its limit",
    ))
}

fn charge_logical(
    current: u64,
    add: u64,
    config: &Config,
) -> Result<u64, Error> {
    let next = current.checked_add(add).ok_or_else(|| {
        Error::new(
            "PREPARED_SCRATCH_LOGICAL_OVERFLOW",
            "scratch logical byte count overflow",
        )
    })?;
    if next <= config.resource_limits.output_file_bytes {
        return Ok(next);
    }
    Err(Error::new(
        "PREPARED_SCRATCH_LOGICAL_LIMIT",
        "scratch logical bytes exceed the output-file limit",
    ))
}

fn push_children(
    path: &Path,
    pending: &mut Vec<PathBuf>,
    config: &Config,
) -> Result<(), Error> {
    let entries = fs::read_dir(path).map_err(|failure| {
        Error::at(
            "PREPARED_SCRATCH_WALK",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|failure| {
            Error::new("PREPARED_SCRATCH_WALK", failure.to_string())
        })?;
        let child = entry.path();
        let text = child.to_str().ok_or_else(|| {
            Error::new("PREPARED_SCRATCH_PATH_UTF8", "path is not UTF-8")
        })?;
        if text.len() > config.inventory_limits.path_bytes {
            return Err(Error::new(
                "PREPARED_SCRATCH_PATH_LIMIT",
                "scratch path exceeds its byte limit",
            ));
        }
        pending.push(child);
    }
    Ok(())
}

fn scratch_kind_error(path: &Path) -> Error {
    Error::at(
        "PREPARED_SCRATCH_ENTRY_KIND",
        path.display().to_string(),
        "scratch contains a symlink or special entry",
    )
}
