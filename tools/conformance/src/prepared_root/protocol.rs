//! Bounded closed JSON for hidden same-binary dispatch.

use std::{path::PathBuf, time::Duration};

use serde_json::{Map, Value, json};

use crate::json as bounded_json;

use super::{
    inventory,
    model::{
        Config, ConfiguredExposure, DispatchBounds, Error, ExposureKind,
        ExposureObservation, ExposurePlan, InventoryEntry, InventoryEntryKind,
        InventoryIdentity, InventoryLimits, MountFact, MountFlags,
        NamespaceFact, NamespaceLinks, NamespaceObservation,
        NetworkObservation, Pid1Result, ResourceLimits, ScratchLimits,
        ScratchObservation, SetupCleanup, SetupFact, SetupResult,
        WorkloadObservation,
    },
};

const CONFIG_SCHEMA: &str = "reovim-prepared-root-config/v1";
const PID1_SCHEMA: &str = "reovim-prepared-root-pid1-result/v1";
const SETUP_SCHEMA: &str = "reovim-prepared-root-setup-result/v1";

pub(super) fn encode_config(config: &Config) -> Result<Vec<u8>, Error> {
    let exposures = config
        .exposures
        .iter()
        .map(configured_exposure_value)
        .collect::<Result<Vec<_>, Error>>()?;
    let value = json!({
        "schema": CONFIG_SCHEMA,
        "mountpoint": path_text(&config.mountpoint)?,
        "repository_root": path_text(&config.repository_root)?,
        "execution_environment": config.execution_environment,
        "workload_request": config.workload_request,
        "checker_executable": path_text(&config.checker_executable)?,
        "exposures": exposures,
        "inventory_limits": inventory_limits_value(config.inventory_limits),
        "scratch_limits": scratch_limits_value(config.scratch_limits),
        "resource_limits": resource_limits_value(config.resource_limits),
        "dispatch_bounds": dispatch_bounds_value(config.dispatch_bounds)?,
        "workload_result_bytes": config.workload_result_bytes,
        "descendant_reap_limit": config.descendant_reap_limit,
        "parent_namespaces": namespace_links_value(&config.parent_namespaces),
        "outside_uid": config.outside_uid,
        "outside_gid": config.outside_gid,
    });
    encode_value(&value, config.dispatch_bounds.config_bytes, "CONFIG")
}

pub(super) fn parse_config(
    bytes: &[u8],
    bootstrap: DispatchBounds,
) -> Result<Config, Error> {
    validate_dispatch_bounds(
        bootstrap,
        "PREPARED_CONFIG_BOOTSTRAP_BOUNDS",
        "bootstrap dispatch bounds are internally invalid",
    )?;
    let value = parse_value(bytes, bootstrap, "CONFIG")?;
    let object = closed_object(
        &value,
        &[
            "schema",
            "mountpoint",
            "repository_root",
            "execution_environment",
            "workload_request",
            "checker_executable",
            "exposures",
            "inventory_limits",
            "scratch_limits",
            "resource_limits",
            "dispatch_bounds",
            "workload_result_bytes",
            "descendant_reap_limit",
            "parent_namespaces",
            "outside_uid",
            "outside_gid",
        ],
        "CONFIG",
    )?;
    exact_schema(object, CONFIG_SCHEMA, "CONFIG")?;
    let parsed_bounds =
        parse_dispatch_bounds(field(object, "dispatch_bounds")?)?;
    validate_selected_bounds(parsed_bounds, bootstrap)?;
    let selected_value = parse_value(bytes, parsed_bounds, "CONFIG")?;
    if selected_value != value {
        return Err(Error::new(
            "PREPARED_CONFIG_SELECTED_REPARSE",
            "selected-bound reparse changed the config value",
        ));
    }
    let exposures = array(field(object, "exposures")?, "CONFIG_EXPOSURES")?
        .iter()
        .map(parse_configured_exposure)
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(Config {
        mountpoint: path(field(object, "mountpoint")?, "CONFIG_MOUNTPOINT")?,
        repository_root: path(
            field(object, "repository_root")?,
            "CONFIG_REPOSITORY_ROOT",
        )?,
        execution_environment: string(
            field(object, "execution_environment")?,
            "CONFIG_ENVIRONMENT",
        )?,
        workload_request: field(object, "workload_request")?.clone(),
        checker_executable: path(
            field(object, "checker_executable")?,
            "CONFIG_CHECKER",
        )?,
        exposures,
        inventory_limits: parse_inventory_limits(field(
            object,
            "inventory_limits",
        )?)?,
        scratch_limits: parse_scratch_limits(field(object, "scratch_limits")?)?,
        resource_limits: parse_resource_limits(field(
            object,
            "resource_limits",
        )?)?,
        dispatch_bounds: parsed_bounds,
        workload_result_bytes: usize_value(
            field(object, "workload_result_bytes")?,
            "CONFIG_WORKLOAD_LIMIT",
        )?,
        descendant_reap_limit: usize_value(
            field(object, "descendant_reap_limit")?,
            "CONFIG_REAP_LIMIT",
        )?,
        parent_namespaces: parse_namespace_links(field(
            object,
            "parent_namespaces",
        )?)?,
        outside_uid: u32_field(object, "outside_uid", "CONFIG_OUTSIDE_UID")?,
        outside_gid: u32_field(object, "outside_gid", "CONFIG_OUTSIDE_GID")?,
    })
}

fn validate_selected_bounds(
    selected: DispatchBounds,
    bootstrap: DispatchBounds,
) -> Result<(), Error> {
    validate_dispatch_bounds(
        selected,
        "PREPARED_CONFIG_SELECTED_BOUNDS",
        "selected dispatch bounds are internally invalid",
    )?;
    if !dispatch_bounds_within(selected, bootstrap) {
        return Err(Error::new(
            "PREPARED_CONFIG_BOUND_EXPANSION",
            "selected dispatch bounds exceed the bootstrap ceiling",
        ));
    }
    Ok(())
}

fn validate_dispatch_bounds(
    value: DispatchBounds,
    code: &'static str,
    message: &'static str,
) -> Result<(), Error> {
    let valid = value.config_bytes > 0
        && value.result_bytes > 0
        && value.json_depth > 0
        && value.json_items > 0
        && !value.execution_deadline.is_zero()
        && value.supervision_deadline > value.execution_deadline;
    if valid {
        return Ok(());
    }
    Err(Error::new(code, message))
}

fn dispatch_bounds_within(
    selected: DispatchBounds,
    bootstrap: DispatchBounds,
) -> bool {
    selected.config_bytes <= bootstrap.config_bytes
        && selected.result_bytes <= bootstrap.result_bytes
        && selected.json_depth <= bootstrap.json_depth
        && selected.json_items <= bootstrap.json_items
        && selected.execution_deadline <= bootstrap.execution_deadline
        && selected.supervision_deadline <= bootstrap.supervision_deadline
}

pub(super) fn encode_pid1_result(
    result: &Pid1Result,
    bounds: DispatchBounds,
) -> Result<Vec<u8>, Error> {
    let value = json!({
        "schema": PID1_SCHEMA,
        "namespaces": namespace_value(&result.namespaces),
        "network_before": network_value(&result.network_before),
        "network_after": network_value(&result.network_after),
        "scratch": scratch_value(&result.scratch),
        "workload": workload_value(&result.workload),
        "descendants_reaped": result.descendants_reaped,
    });
    encode_value(&value, bounds.result_bytes, "PID1_RESULT")
}

pub(super) fn parse_pid1_result(
    bytes: &[u8],
    bounds: DispatchBounds,
) -> Result<Pid1Result, Error> {
    let value = parse_value(bytes, bounds, "PID1_RESULT")?;
    let object = closed_object(
        &value,
        &[
            "schema",
            "namespaces",
            "network_before",
            "network_after",
            "scratch",
            "workload",
            "descendants_reaped",
        ],
        "PID1_RESULT",
    )?;
    exact_schema(object, PID1_SCHEMA, "PID1_RESULT")?;
    Ok(Pid1Result {
        namespaces: parse_namespace(field(object, "namespaces")?)?,
        network_before: parse_network(field(object, "network_before")?)?,
        network_after: parse_network(field(object, "network_after")?)?,
        scratch: parse_scratch(field(object, "scratch")?)?,
        workload: parse_workload(field(object, "workload")?)?,
        descendants_reaped: boolean(
            field(object, "descendants_reaped")?,
            "PID1_REAPED",
        )?,
    })
}

pub(super) fn encode_setup_result(
    result: &SetupResult,
    bounds: DispatchBounds,
) -> Result<Vec<u8>, Error> {
    let exposures =
        result.exposures.iter().map(exposure_value).collect::<Vec<_>>();
    let value = json!({
        "schema": SETUP_SCHEMA,
        "exposures": exposures,
        "pid1": pid1_value(&result.pid1),
        "readers_joined": result.cleanup.readers_joined(),
        "proc_unmounted": result.cleanup.proc_unmounted(),
        "exposures_unmounted": result.cleanup.exposures_unmounted(),
        "scratch_unmounted": result.cleanup.scratch_unmounted(),
        "source_identity_preserved":
            result.cleanup.source_identity_preserved(),
        "tool_identity_preserved": result.cleanup.tool_identity_preserved(),
    });
    encode_value(&value, bounds.result_bytes, "SETUP_RESULT")
}

pub(super) fn parse_setup_result(
    bytes: &[u8],
    bounds: DispatchBounds,
) -> Result<SetupResult, Error> {
    let value = parse_value(bytes, bounds, "SETUP_RESULT")?;
    let object = closed_object(
        &value,
        &[
            "schema",
            "exposures",
            "pid1",
            "readers_joined",
            "proc_unmounted",
            "exposures_unmounted",
            "scratch_unmounted",
            "source_identity_preserved",
            "tool_identity_preserved",
        ],
        "SETUP_RESULT",
    )?;
    exact_schema(object, SETUP_SCHEMA, "SETUP_RESULT")?;
    let exposures = array(field(object, "exposures")?, "SETUP_EXPOSURES")?
        .iter()
        .map(parse_exposure)
        .collect::<Result<Vec<_>, Error>>()?;
    let mut cleanup = SetupCleanup::empty();
    record_setup(
        &mut cleanup,
        SetupFact::ReadersJoined,
        object,
        "readers_joined",
        "SETUP_READERS",
    )?;
    record_setup(
        &mut cleanup,
        SetupFact::ProcUnmounted,
        object,
        "proc_unmounted",
        "SETUP_PROC",
    )?;
    record_setup(
        &mut cleanup,
        SetupFact::ExposuresUnmounted,
        object,
        "exposures_unmounted",
        "SETUP_EXPOSURES",
    )?;
    record_setup(
        &mut cleanup,
        SetupFact::ScratchUnmounted,
        object,
        "scratch_unmounted",
        "SETUP_SCRATCH",
    )?;
    record_setup(
        &mut cleanup,
        SetupFact::SourceIdentityPreserved,
        object,
        "source_identity_preserved",
        "SETUP_SOURCE",
    )?;
    record_setup(
        &mut cleanup,
        SetupFact::ToolIdentityPreserved,
        object,
        "tool_identity_preserved",
        "SETUP_TOOL",
    )?;
    Ok(SetupResult {
        exposures,
        pid1: parse_pid1_value(field(object, "pid1")?)?,
        cleanup,
    })
}

fn record_setup(
    cleanup: &mut SetupCleanup,
    fact: SetupFact,
    object: &Map<String, Value>,
    name: &'static str,
    context: &'static str,
) -> Result<(), Error> {
    cleanup.record(fact, bool_field(object, name, context)?);
    Ok(())
}

pub(super) const fn ready_frame() -> &'static [u8] {
    b"{\"schema\":\"reovim-prepared-root-ready/v1\"}\n"
}

fn configured_exposure_value(
    exposure: &ConfiguredExposure,
) -> Result<Value, Error> {
    Ok(json!({
        "id": exposure.plan.id,
        "source": path_text(&exposure.plan.source)?,
        "destination": path_text(&exposure.plan.destination)?,
        "kind": exposure.plan.kind.as_str(),
        "executable": exposure.plan.executable,
        "purpose": exposure.plan.purpose,
        "expected": inventory_value(&exposure.expected),
    }))
}

fn parse_configured_exposure(
    value: &Value,
) -> Result<ConfiguredExposure, Error> {
    let object = closed_object(
        value,
        &[
            "id",
            "source",
            "destination",
            "kind",
            "executable",
            "purpose",
            "expected",
        ],
        "CONFIG_EXPOSURE",
    )?;
    Ok(ConfiguredExposure {
        plan: ExposurePlan {
            id: string(field(object, "id")?, "CONFIG_EXPOSURE_ID")?,
            source: path(field(object, "source")?, "CONFIG_EXPOSURE_SOURCE")?,
            destination: path(
                field(object, "destination")?,
                "CONFIG_EXPOSURE_DESTINATION",
            )?,
            kind: exposure_kind(field(object, "kind")?)?,
            executable: boolean(
                field(object, "executable")?,
                "CONFIG_EXPOSURE_EXECUTABLE",
            )?,
            purpose: string(
                field(object, "purpose")?,
                "CONFIG_EXPOSURE_PURPOSE",
            )?,
        },
        expected: parse_inventory(field(object, "expected")?)?,
    })
}

fn inventory_limits_value(limits: InventoryLimits) -> Value {
    json!({
        "path_bytes": limits.path_bytes,
        "entry_count": limits.entry_count,
        "total_bytes": limits.total_bytes,
    })
}

fn parse_inventory_limits(value: &Value) -> Result<InventoryLimits, Error> {
    let object = closed_object(
        value,
        &["path_bytes", "entry_count", "total_bytes"],
        "CONFIG_INVENTORY_LIMITS",
    )?;
    Ok(InventoryLimits {
        path_bytes: usize_field(object, "path_bytes", "CONFIG_PATH_LIMIT")?,
        entry_count: usize_field(object, "entry_count", "CONFIG_ENTRY_LIMIT")?,
        total_bytes: u64_field(object, "total_bytes", "CONFIG_BYTE_LIMIT")?,
    })
}

fn scratch_limits_value(limits: ScratchLimits) -> Value {
    json!({
        "size_bytes": limits.size_bytes,
        "inode_count": limits.inode_count,
    })
}

fn parse_scratch_limits(value: &Value) -> Result<ScratchLimits, Error> {
    let object = closed_object(
        value,
        &["size_bytes", "inode_count"],
        "CONFIG_SCRATCH_LIMITS",
    )?;
    Ok(ScratchLimits {
        size_bytes: u64_field(object, "size_bytes", "CONFIG_SCRATCH_BYTES")?,
        inode_count: u64_field(object, "inode_count", "CONFIG_SCRATCH_INODES")?,
    })
}

fn resource_limits_value(limits: ResourceLimits) -> Value {
    json!({
        "address_space_bytes": limits.address_space_bytes,
        "cpu_seconds": limits.cpu_seconds,
        "open_files": limits.open_files,
        "output_file_bytes": limits.output_file_bytes,
        "core_dump_bytes": limits.core_dump_bytes,
    })
}

fn parse_resource_limits(value: &Value) -> Result<ResourceLimits, Error> {
    let object = closed_object(
        value,
        &[
            "address_space_bytes",
            "cpu_seconds",
            "open_files",
            "output_file_bytes",
            "core_dump_bytes",
        ],
        "CONFIG_RESOURCE_LIMITS",
    )?;
    Ok(ResourceLimits {
        address_space_bytes: u64_field(
            object,
            "address_space_bytes",
            "CONFIG_RLIMIT_AS",
        )?,
        cpu_seconds: u64_field(object, "cpu_seconds", "CONFIG_RLIMIT_CPU")?,
        open_files: u64_field(object, "open_files", "CONFIG_RLIMIT_NOFILE")?,
        output_file_bytes: u64_field(
            object,
            "output_file_bytes",
            "CONFIG_RLIMIT_FSIZE",
        )?,
        core_dump_bytes: u64_field(
            object,
            "core_dump_bytes",
            "CONFIG_RLIMIT_CORE",
        )?,
    })
}

fn dispatch_bounds_value(bounds: DispatchBounds) -> Result<Value, Error> {
    Ok(json!({
        "config_bytes": bounds.config_bytes,
        "result_bytes": bounds.result_bytes,
        "json_depth": bounds.json_depth,
        "json_items": bounds.json_items,
        "execution_deadline_ms": duration_millis(
            bounds.execution_deadline,
        )?,
        "supervision_deadline_ms": duration_millis(
            bounds.supervision_deadline,
        )?,
    }))
}

fn parse_dispatch_bounds(value: &Value) -> Result<DispatchBounds, Error> {
    let object = closed_object(
        value,
        &[
            "config_bytes",
            "result_bytes",
            "json_depth",
            "json_items",
            "execution_deadline_ms",
            "supervision_deadline_ms",
        ],
        "CONFIG_DISPATCH_BOUNDS",
    )?;
    Ok(DispatchBounds {
        config_bytes: usize_field(object, "config_bytes", "CONFIG_BYTES")?,
        result_bytes: usize_field(object, "result_bytes", "RESULT_BYTES")?,
        json_depth: usize_field(object, "json_depth", "CONFIG_JSON_DEPTH")?,
        json_items: usize_field(object, "json_items", "CONFIG_JSON_ITEMS")?,
        execution_deadline: duration_field(
            object,
            "execution_deadline_ms",
            "CONFIG_EXECUTION_DEADLINE",
        )?,
        supervision_deadline: duration_field(
            object,
            "supervision_deadline_ms",
            "CONFIG_SUPERVISION_DEADLINE",
        )?,
    })
}

fn namespace_links_value(links: &NamespaceLinks) -> Value {
    json!({
        "user": links.user,
        "mount": links.mount,
        "network": links.network,
        "pid": links.pid,
    })
}

fn parse_namespace_links(value: &Value) -> Result<NamespaceLinks, Error> {
    let object = closed_object(
        value,
        &["user", "mount", "network", "pid"],
        "CONFIG_PARENT_NAMESPACES",
    )?;
    Ok(NamespaceLinks {
        user: string_field(object, "user", "CONFIG_USER_NAMESPACE")?,
        mount: string_field(object, "mount", "CONFIG_MOUNT_NAMESPACE")?,
        network: string_field(object, "network", "CONFIG_NETWORK_NAMESPACE")?,
        pid: string_field(object, "pid", "CONFIG_PID_NAMESPACE")?,
    })
}

fn inventory_value(identity: &InventoryIdentity) -> Value {
    let entries = identity
        .entries
        .iter()
        .map(|entry| {
            json!({
                "relative_path": entry.relative_path,
                "kind": entry.kind.as_str(),
                "mode": entry.mode,
                "size_bytes": entry.size_bytes,
                "content_digest": entry.content_digest,
                "device_identity": entry.device_identity,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "entry_count": identity.entry_count,
        "total_bytes": identity.total_bytes,
        "aggregate_digest": identity.aggregate_digest,
        "entries": entries,
    })
}

fn parse_inventory(value: &Value) -> Result<InventoryIdentity, Error> {
    let object = closed_object(
        value,
        &["entry_count", "total_bytes", "aggregate_digest", "entries"],
        "INVENTORY_IDENTITY",
    )?;
    let entries = array(field(object, "entries")?, "INVENTORY_ENTRIES")?
        .iter()
        .map(parse_inventory_entry)
        .collect::<Result<Vec<_>, _>>()?;
    let identity = InventoryIdentity {
        entry_count: usize_field(object, "entry_count", "INVENTORY_COUNT")?,
        total_bytes: u64_field(object, "total_bytes", "INVENTORY_BYTES")?,
        aggregate_digest: string_field(
            object,
            "aggregate_digest",
            "INVENTORY_DIGEST",
        )?,
        entries,
    };
    inventory::validate_identity(&identity)?;
    Ok(identity)
}

fn parse_inventory_entry(value: &Value) -> Result<InventoryEntry, Error> {
    let object = closed_object(
        value,
        &[
            "relative_path",
            "kind",
            "mode",
            "size_bytes",
            "content_digest",
            "device_identity",
        ],
        "INVENTORY_ENTRY",
    )?;
    let kind =
        match string_field(object, "kind", "INVENTORY_ENTRY_KIND")?.as_str() {
            "file" => InventoryEntryKind::File,
            "directory" => InventoryEntryKind::Directory,
            "character_device" => InventoryEntryKind::CharacterDevice,
            _ => {
                return Err(Error::new(
                    "PREPARED_INVENTORY_ENTRY_KIND",
                    "inventory entry has an unknown kind",
                ));
            }
        };
    Ok(InventoryEntry {
        relative_path: PathBuf::from(string_field(
            object,
            "relative_path",
            "INVENTORY_ENTRY_PATH",
        )?),
        kind,
        mode: u32_field(object, "mode", "INVENTORY_ENTRY_MODE")?,
        size_bytes: u64_field(object, "size_bytes", "INVENTORY_ENTRY_BYTES")?,
        content_digest: optional_string(
            field(object, "content_digest")?,
            "INVENTORY_ENTRY_DIGEST",
        )?,
        device_identity: optional_u64(
            field(object, "device_identity")?,
            "INVENTORY_ENTRY_DEVICE",
        )?,
    })
}

fn exposure_value(exposure: &ExposureObservation) -> Value {
    json!({
        "id": exposure.id,
        "destination": exposure.destination,
        "kind": exposure.kind.as_str(),
        "executable": exposure.executable,
        "purpose": exposure.purpose,
        "inventory": inventory_value(&exposure.inventory),
        "device_major": exposure.device_major,
        "device_minor": exposure.device_minor,
        "read_only": exposure.mount_flags.read_only(),
        "nosuid": exposure.mount_flags.nosuid(),
        "nodev": exposure.mount_flags.nodev(),
        "noexec": exposure.mount_flags.noexec(),
    })
}

fn parse_exposure(value: &Value) -> Result<ExposureObservation, Error> {
    let object = closed_object(
        value,
        &[
            "id",
            "destination",
            "kind",
            "executable",
            "purpose",
            "inventory",
            "device_major",
            "device_minor",
            "read_only",
            "nosuid",
            "nodev",
            "noexec",
        ],
        "EXPOSURE_OBSERVATION",
    )?;
    let mount_flags = parse_mount_flags(object)?;
    Ok(ExposureObservation {
        id: string_field(object, "id", "EXPOSURE_ID")?,
        destination: path(field(object, "destination")?, "EXPOSURE_PATH")?,
        kind: exposure_kind(field(object, "kind")?)?,
        executable: bool_field(object, "executable", "EXPOSURE_EXECUTABLE")?,
        purpose: string_field(object, "purpose", "EXPOSURE_PURPOSE")?,
        inventory: parse_inventory(field(object, "inventory")?)?,
        device_major: optional_u64(
            field(object, "device_major")?,
            "EXPOSURE_DEVICE_MAJOR",
        )?,
        device_minor: optional_u64(
            field(object, "device_minor")?,
            "EXPOSURE_DEVICE_MINOR",
        )?,
        mount_flags,
    })
}

fn parse_scratch_mount_flags(
    object: &Map<String, Value>,
) -> Result<MountFlags, Error> {
    let mut flags = MountFlags::empty();
    for (fact, name, context) in [
        (MountFact::Nosuid, "nosuid", "SCRATCH_NOSUID"),
        (MountFact::Nodev, "nodev", "SCRATCH_NODEV"),
        (MountFact::Noexec, "noexec", "SCRATCH_NOEXEC"),
    ] {
        flags.record(fact, bool_field(object, name, context)?);
    }
    Ok(flags)
}

fn parse_mount_flags(object: &Map<String, Value>) -> Result<MountFlags, Error> {
    let mut flags = MountFlags::empty();
    for (fact, name, context) in [
        (MountFact::ReadOnly, "read_only", "MOUNT_READ_ONLY"),
        (MountFact::Nosuid, "nosuid", "MOUNT_NOSUID"),
        (MountFact::Nodev, "nodev", "MOUNT_NODEV"),
        (MountFact::Noexec, "noexec", "MOUNT_NOEXEC"),
    ] {
        flags.record(fact, bool_field(object, name, context)?);
    }
    Ok(flags)
}

fn namespace_value(value: &NamespaceObservation) -> Value {
    json!({
        "user_private": value.user_private(),
        "mount_private": value.mount_private(),
        "network_private": value.network_private(),
        "pid_private": value.pid_private(),
        "uid_map_single": value.uid_map_single(),
        "gid_map_single": value.gid_map_single(),
        "uid_map": value.uid_map,
        "gid_map": value.gid_map,
        "setgroups_denied": value.setgroups_denied(),
        "private_propagation": value.private_propagation(),
        "pid_one": value.pid_one(),
        "parent_death_signal": value.parent_death_signal(),
    })
}

fn parse_namespace(value: &Value) -> Result<NamespaceObservation, Error> {
    let object = closed_object(
        value,
        &[
            "user_private",
            "mount_private",
            "network_private",
            "pid_private",
            "uid_map_single",
            "gid_map_single",
            "uid_map",
            "gid_map",
            "setgroups_denied",
            "private_propagation",
            "pid_one",
            "parent_death_signal",
        ],
        "NAMESPACE_OBSERVATION",
    )?;
    let uid_map = string_field(object, "uid_map", "NAMESPACE_UID_MAP")?;
    let gid_map = string_field(object, "gid_map", "NAMESPACE_GID_MAP")?;
    let mut observation = NamespaceObservation::empty(uid_map, gid_map);
    for (fact, name, context) in [
        (NamespaceFact::UserPrivate, "user_private", "NAMESPACE_USER"),
        (NamespaceFact::MountPrivate, "mount_private", "NAMESPACE_MOUNT"),
        (NamespaceFact::NetworkPrivate, "network_private", "NAMESPACE_NETWORK"),
        (NamespaceFact::PidPrivate, "pid_private", "NAMESPACE_PID"),
        (NamespaceFact::UidMapSingle, "uid_map_single", "NAMESPACE_UID"),
        (NamespaceFact::GidMapSingle, "gid_map_single", "NAMESPACE_GID"),
        (
            NamespaceFact::SetgroupsDenied,
            "setgroups_denied",
            "NAMESPACE_SETGROUPS",
        ),
        (
            NamespaceFact::PrivatePropagation,
            "private_propagation",
            "NAMESPACE_PROPAGATION",
        ),
        (NamespaceFact::PidOne, "pid_one", "NAMESPACE_PID_ONE"),
        (
            NamespaceFact::ParentDeathSignal,
            "parent_death_signal",
            "NAMESPACE_PDEATH",
        ),
    ] {
        observation.record(fact, bool_field(object, name, context)?);
    }
    Ok(observation)
}

fn network_value(value: &NetworkObservation) -> Value {
    json!({
        "interfaces": value.interfaces,
        "ipv4_route_count": value.ipv4_route_count,
        "ipv6_route_count": value.ipv6_route_count,
        "kernel_route_count": value.kernel_route_count,
        "non_packet_socket_count": value.non_packet_socket_count,
        "packet_socket_count": value.packet_socket_count,
        "policy_observation_digest": value.policy_observation_digest,
        "policy_basis": value.policy_basis,
        "external_carrier_absent": value.external_carrier_absent,
    })
}

fn parse_network(value: &Value) -> Result<NetworkObservation, Error> {
    let object = closed_object(
        value,
        &[
            "interfaces",
            "ipv4_route_count",
            "ipv6_route_count",
            "kernel_route_count",
            "non_packet_socket_count",
            "packet_socket_count",
            "policy_observation_digest",
            "policy_basis",
            "external_carrier_absent",
        ],
        "NETWORK_OBSERVATION",
    )?;
    let interfaces = string_array(field(object, "interfaces")?, "INTERFACES")?;
    Ok(NetworkObservation {
        interfaces,
        ipv4_route_count: usize_field(
            object,
            "ipv4_route_count",
            "IPV4_ROUTES",
        )?,
        ipv6_route_count: usize_field(
            object,
            "ipv6_route_count",
            "IPV6_ROUTES",
        )?,
        kernel_route_count: usize_field(
            object,
            "kernel_route_count",
            "KERNEL_ROUTES",
        )?,
        non_packet_socket_count: usize_field(
            object,
            "non_packet_socket_count",
            "NON_PACKET_SOCKETS",
        )?,
        packet_socket_count: usize_field(
            object,
            "packet_socket_count",
            "PACKET_SOCKETS",
        )?,
        policy_observation_digest: string_field(
            object,
            "policy_observation_digest",
            "NETWORK_POLICY_DIGEST",
        )?,
        policy_basis: string_field(
            object,
            "policy_basis",
            "NETWORK_POLICY_BASIS",
        )?,
        external_carrier_absent: bool_field(
            object,
            "external_carrier_absent",
            "NETWORK_CARRIER",
        )?,
    })
}

fn scratch_value(value: &ScratchObservation) -> Value {
    json!({
        "size_limit_bytes": value.size_limit_bytes,
        "inode_limit": value.inode_limit,
        "allocated_bytes": value.allocated_bytes,
        "inode_count": value.inode_count,
        "logical_bytes": value.logical_bytes,
        "mode_owner_only": value.mode_owner_only,
        "nosuid": value.mount_flags.nosuid(),
        "nodev": value.mount_flags.nodev(),
        "noexec": value.mount_flags.noexec(),
    })
}

fn parse_scratch(value: &Value) -> Result<ScratchObservation, Error> {
    let object = closed_object(
        value,
        &[
            "size_limit_bytes",
            "inode_limit",
            "allocated_bytes",
            "inode_count",
            "logical_bytes",
            "mode_owner_only",
            "nosuid",
            "nodev",
            "noexec",
        ],
        "SCRATCH_OBSERVATION",
    )?;
    let mount_flags = parse_scratch_mount_flags(object)?;
    Ok(ScratchObservation {
        size_limit_bytes: u64_field(
            object,
            "size_limit_bytes",
            "SCRATCH_SIZE",
        )?,
        inode_limit: u64_field(object, "inode_limit", "SCRATCH_INODE_LIMIT")?,
        allocated_bytes: u64_field(
            object,
            "allocated_bytes",
            "SCRATCH_ALLOCATED",
        )?,
        inode_count: u64_field(object, "inode_count", "SCRATCH_INODES")?,
        logical_bytes: u64_field(object, "logical_bytes", "SCRATCH_LOGICAL")?,
        mode_owner_only: bool_field(object, "mode_owner_only", "SCRATCH_MODE")?,
        mount_flags,
    })
}

fn workload_value(value: &WorkloadObservation) -> Value {
    json!({
        "status": value.status,
        "bytes": value.bytes,
        "digest": value.digest,
        "value": value.value,
    })
}

fn parse_workload(value: &Value) -> Result<WorkloadObservation, Error> {
    let object = closed_object(
        value,
        &["status", "bytes", "digest", "value"],
        "WORKLOAD_OBSERVATION",
    )?;
    Ok(WorkloadObservation {
        status: string_field(object, "status", "WORKLOAD_STATUS")?,
        bytes: u64_field(object, "bytes", "WORKLOAD_BYTES")?,
        digest: string_field(object, "digest", "WORKLOAD_DIGEST")?,
        value: field(object, "value")?.clone(),
    })
}

fn pid1_value(value: &Pid1Result) -> Value {
    json!({
        "namespaces": namespace_value(&value.namespaces),
        "network_before": network_value(&value.network_before),
        "network_after": network_value(&value.network_after),
        "scratch": scratch_value(&value.scratch),
        "workload": workload_value(&value.workload),
        "descendants_reaped": value.descendants_reaped,
    })
}

fn parse_pid1_value(value: &Value) -> Result<Pid1Result, Error> {
    let object = closed_object(
        value,
        &[
            "namespaces",
            "network_before",
            "network_after",
            "scratch",
            "workload",
            "descendants_reaped",
        ],
        "SETUP_PID1",
    )?;
    Ok(Pid1Result {
        namespaces: parse_namespace(field(object, "namespaces")?)?,
        network_before: parse_network(field(object, "network_before")?)?,
        network_after: parse_network(field(object, "network_after")?)?,
        scratch: parse_scratch(field(object, "scratch")?)?,
        workload: parse_workload(field(object, "workload")?)?,
        descendants_reaped: bool_field(
            object,
            "descendants_reaped",
            "SETUP_REAPED",
        )?,
    })
}

fn parse_value(
    bytes: &[u8],
    bounds: DispatchBounds,
    context: &'static str,
) -> Result<Value, Error> {
    let limit = if context == "CONFIG" {
        bounds.config_bytes
    } else {
        bounds.result_bytes
    };
    if bytes.len() > limit {
        return Err(Error::new(
            "PREPARED_JSON_BYTE_LIMIT",
            format!("{context} exceeds {limit} bytes"),
        ));
    }
    bounded_json::parse(bytes, bounds.json_depth, bounds.json_items).map_err(
        |failure| Error::new("PREPARED_JSON_PARSE", failure.to_string()),
    )
}

fn encode_value(
    value: &Value,
    limit: usize,
    context: &'static str,
) -> Result<Vec<u8>, Error> {
    let bytes = serde_json::to_vec(value).map_err(|failure| {
        Error::new("PREPARED_JSON_ENCODE", failure.to_string())
    })?;
    if bytes.len() <= limit {
        return Ok(bytes);
    }
    Err(Error::new(
        "PREPARED_JSON_BYTE_LIMIT",
        format!("{context} exceeds {limit} bytes"),
    ))
}

fn closed_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &'static str,
) -> Result<&'a Map<String, Value>, Error> {
    let object = value.as_object().ok_or_else(|| {
        Error::new("PREPARED_JSON_TYPE", format!("{context} is not an object"))
    })?;
    if object.len() != fields.len()
        || object.keys().any(|key| !fields.contains(&key.as_str()))
    {
        return Err(Error::new(
            "PREPARED_JSON_FIELDS",
            format!("{context} has unknown or missing fields"),
        ));
    }
    Ok(object)
}

fn exact_schema(
    object: &Map<String, Value>,
    expected: &str,
    context: &'static str,
) -> Result<(), Error> {
    let actual = string_field(object, "schema", context)?;
    if actual == expected {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_JSON_SCHEMA",
        format!("{context} has schema {actual:?}"),
    ))
}

fn field<'a>(
    object: &'a Map<String, Value>,
    name: &'static str,
) -> Result<&'a Value, Error> {
    object.get(name).ok_or_else(|| {
        Error::new("PREPARED_JSON_FIELD", format!("missing field {name}"))
    })
}

fn string(value: &Value, context: &'static str) -> Result<String, Error> {
    let text = value.as_str().ok_or_else(|| {
        Error::new("PREPARED_JSON_TYPE", format!("{context} is not a string"))
    })?;
    if !text.is_empty() && !text.contains(['\0', '\r', '\n']) {
        return Ok(text.to_owned());
    }
    Err(Error::new(
        "PREPARED_JSON_STRING",
        format!("{context} is empty or contains control framing"),
    ))
}

fn string_field(
    object: &Map<String, Value>,
    name: &'static str,
    context: &'static str,
) -> Result<String, Error> {
    string(field(object, name)?, context)
}

fn boolean(value: &Value, context: &'static str) -> Result<bool, Error> {
    value.as_bool().ok_or_else(|| {
        Error::new("PREPARED_JSON_TYPE", format!("{context} is not boolean"))
    })
}

fn bool_field(
    object: &Map<String, Value>,
    name: &'static str,
    context: &'static str,
) -> Result<bool, Error> {
    boolean(field(object, name)?, context)
}

fn u64_value(value: &Value, context: &'static str) -> Result<u64, Error> {
    value.as_u64().ok_or_else(|| {
        Error::new(
            "PREPARED_JSON_TYPE",
            format!("{context} is not an unsigned integer"),
        )
    })
}

fn optional_u64(
    value: &Value,
    context: &'static str,
) -> Result<Option<u64>, Error> {
    if value.is_null() {
        return Ok(None);
    }
    u64_value(value, context).map(Some)
}

fn optional_string(
    value: &Value,
    context: &'static str,
) -> Result<Option<String>, Error> {
    if value.is_null() {
        return Ok(None);
    }
    string(value, context).map(Some)
}

fn u64_field(
    object: &Map<String, Value>,
    name: &'static str,
    context: &'static str,
) -> Result<u64, Error> {
    u64_value(field(object, name)?, context)
}

fn usize_value(value: &Value, context: &'static str) -> Result<usize, Error> {
    usize::try_from(u64_value(value, context)?).map_err(|failure| {
        Error::new("PREPARED_JSON_RANGE", failure.to_string())
    })
}

fn u32_field(
    object: &Map<String, Value>,
    name: &'static str,
    context: &'static str,
) -> Result<u32, Error> {
    u32::try_from(u64_field(object, name, context)?).map_err(|failure| {
        Error::new("PREPARED_JSON_RANGE", failure.to_string())
    })
}

fn usize_field(
    object: &Map<String, Value>,
    name: &'static str,
    context: &'static str,
) -> Result<usize, Error> {
    usize_value(field(object, name)?, context)
}

fn array<'a>(
    value: &'a Value,
    context: &'static str,
) -> Result<&'a [Value], Error> {
    value.as_array().map(Vec::as_slice).ok_or_else(|| {
        Error::new("PREPARED_JSON_TYPE", format!("{context} is not an array"))
    })
}

fn string_array(
    value: &Value,
    context: &'static str,
) -> Result<Vec<String>, Error> {
    array(value, context)?.iter().map(|item| string(item, context)).collect()
}

fn path(value: &Value, context: &'static str) -> Result<PathBuf, Error> {
    string(value, context).map(PathBuf::from)
}

fn path_text(path: &std::path::Path) -> Result<&str, Error> {
    path.to_str().ok_or_else(|| {
        Error::at(
            "PREPARED_JSON_PATH_UTF8",
            path.display().to_string(),
            "path is not UTF-8",
        )
    })
}

fn exposure_kind(value: &Value) -> Result<ExposureKind, Error> {
    match value.as_str() {
        Some("file") => Ok(ExposureKind::File),
        Some("directory") => Ok(ExposureKind::Directory),
        Some("character_device") => Ok(ExposureKind::CharacterDevice),
        _ => Err(Error::new("PREPARED_EXPOSURE_KIND", "unknown exposure kind")),
    }
}

fn duration_millis(duration: Duration) -> Result<u64, Error> {
    u64::try_from(duration.as_millis()).map_err(|failure| {
        Error::new("PREPARED_DURATION_RANGE", failure.to_string())
    })
}

fn duration_field(
    object: &Map<String, Value>,
    name: &'static str,
    context: &'static str,
) -> Result<Duration, Error> {
    Ok(Duration::from_millis(u64_field(object, name, context)?))
}

#[cfg(test)]
pub(super) fn namespace_roundtrip_for_test(
    value: &NamespaceObservation,
) -> Result<NamespaceObservation, Error> {
    parse_namespace(&namespace_value(value))
}

#[cfg(test)]
pub(super) fn network_value_for_test(value: &NetworkObservation) -> Value {
    network_value(value)
}

#[cfg(test)]
pub(super) fn parse_network_for_test(
    value: &Value,
) -> Result<NetworkObservation, Error> {
    parse_network(value)
}
