use std::{
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt as _,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use serde_json::{Value, json};

use super::{
    inventory, linux,
    model::{
        Config, ConfiguredExposure, DispatchBounds, ExposureKind, ExposurePlan,
        InventoryLimits, NamespaceFact, NamespaceLinks, NamespaceObservation,
        NetworkObservation, Plan, ResourceLimits, ScratchLimits,
    },
    network, parent, protocol, setup,
};
use crate::process::{ProcessResult, TerminationReason};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

#[test]
fn plan_accepts_exact_required_exposures() {
    let fixture = Fixture::new();
    let plan = fixture.plan();

    linux::validate_plan_for_test(&plan).unwrap();
}

#[test]
fn plan_accepts_checker_file_built_beneath_repository() {
    let fixture = Fixture::new();
    let mut plan = fixture.plan();
    let checker = fixture.repository.join("checker");
    fs::write(&checker, b"checker\n").unwrap();
    fs::set_permissions(&checker, fs::Permissions::from_mode(0o700)).unwrap();
    plan.checker_executable = checker.clone();
    let exposure =
        plan.exposures.iter_mut().find(|value| value.id == "checker").unwrap();
    exposure.source = checker;
    exposure.destination = PathBuf::from("/checker/reovim-conformance");

    linux::validate_plan_for_test(&plan).unwrap();
}

#[test]
fn plan_rejects_missing_required_exposure() {
    let fixture = Fixture::new();
    let mut plan = fixture.plan();
    plan.exposures.retain(|value| value.kind != ExposureKind::CharacterDevice);

    let error = linux::validate_plan_for_test(&plan).unwrap_err();
    assert_eq!(error.code, "PREPARED_REQUIRED_EXPOSURE");
}

#[test]
fn plan_rejects_destination_overlap() {
    let fixture = Fixture::new();
    let mut plan = fixture.plan();
    let mut parent = plan.exposures[0].clone();
    parent.id = "checker-parent".to_owned();
    parent.destination = PathBuf::from("/tool");
    plan.exposures.push(parent);

    let error = linux::validate_plan_for_test(&plan).unwrap_err();
    assert_eq!(error.code, "PREPARED_EXPOSURE_OVERLAP");
}

#[test]
fn plan_rejects_wrong_null_device_mapping() {
    let fixture = Fixture::new();
    let mut plan = fixture.plan();
    let device = plan
        .exposures
        .iter_mut()
        .find(|value| value.kind == ExposureKind::CharacterDevice)
        .unwrap();
    device.destination = PathBuf::from("/dev/zero");

    let error = linux::validate_plan_for_test(&plan).unwrap_err();
    assert_eq!(error.code, "PREPARED_DEVICE_IDENTITY");
}

#[test]
fn plan_rejects_deep_workload_request() {
    let fixture = Fixture::new();
    let mut plan = fixture.plan();
    plan.dispatch_bounds.json_depth = 2;
    plan.workload_request = json!({"one": {"two": {"three": true}}});

    let error = linux::validate_plan_for_test(&plan).unwrap_err();
    assert_eq!(error.code, "PREPARED_REQUEST_JSON");
}

#[test]
fn config_roundtrip_preserves_opaque_workload_request() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let config = fixture.config(&plan);
    let encoded = protocol::encode_config(&config).unwrap();
    let parsed =
        protocol::parse_config(&encoded, plan.dispatch_bounds).unwrap();

    assert_eq!(parsed, config);
    assert_eq!(parsed.workload_request, plan.workload_request);
}

#[test]
fn config_parser_accepts_tightened_selected_bounds() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let config = fixture.config(&plan);
    let encoded = protocol::encode_config(&config).unwrap();
    let bootstrap = expanded_bootstrap(plan.dispatch_bounds);

    let parsed = protocol::parse_config(&encoded, bootstrap).unwrap();

    assert_eq!(parsed.dispatch_bounds, plan.dispatch_bounds);
}

#[test]
fn config_parser_rejects_one_field_expansion() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let config = fixture.config(&plan);
    let encoded = protocol::encode_config(&config).unwrap();
    let mut value: Value = serde_json::from_slice(&encoded).unwrap();
    value["dispatch_bounds"]["json_depth"] =
        json!(plan.dispatch_bounds.json_depth + 1);
    let mutant = serde_json::to_vec(&value).unwrap();

    let error =
        protocol::parse_config(&mutant, plan.dispatch_bounds).unwrap_err();
    assert_eq!(error.code, "PREPARED_CONFIG_BOUND_EXPANSION");
}

#[test]
fn config_parser_rechecks_payload_under_selected_bounds() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let config = fixture.config(&plan);
    let encoded = protocol::encode_config(&config).unwrap();
    let mut value: Value = serde_json::from_slice(&encoded).unwrap();
    value["dispatch_bounds"]["json_depth"] = json!(1);
    let mutant = serde_json::to_vec(&value).unwrap();

    let error =
        protocol::parse_config(&mutant, plan.dispatch_bounds).unwrap_err();
    assert_eq!(error.code, "PREPARED_JSON_PARSE");
}

#[test]
fn config_rejects_oversize_workload_request() {
    let fixture = Fixture::new();
    let mut plan = fixture.plan();
    plan.dispatch_bounds.config_bytes = 128;
    plan.workload_request = Value::String("x".repeat(256));

    let error = linux::validate_plan_for_test(&plan).unwrap_err();
    assert_eq!(error.code, "PREPARED_REQUEST_BYTE_LIMIT");
}

#[test]
fn config_parser_rejects_unknown_field() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let config = fixture.config(&plan);
    let encoded = protocol::encode_config(&config).unwrap();
    let mut value: Value = serde_json::from_slice(&encoded).unwrap();
    value["unexpected"] = Value::Bool(true);
    let mutant = serde_json::to_vec(&value).unwrap();

    let error =
        protocol::parse_config(&mutant, plan.dispatch_bounds).unwrap_err();
    assert_eq!(error.code, "PREPARED_JSON_FIELDS");
}

#[test]
fn namespace_protocol_retains_exact_identifier_maps() {
    let value = complete_namespace("0 1000 1", "0 2000 1");

    let parsed = protocol::namespace_roundtrip_for_test(&value).unwrap();

    assert_eq!(parsed, value);
    assert_eq!(parsed.uid_map, "0 1000 1");
    assert_eq!(parsed.gid_map, "0 2000 1");
}

#[test]
fn network_protocol_names_the_non_packet_aggregate_exactly() {
    let observation = NetworkObservation {
        interfaces: vec!["lo".to_owned()],
        ipv4_route_count: 0,
        ipv6_route_count: 0,
        kernel_route_count: 2,
        non_packet_socket_count: 3,
        packet_socket_count: 1,
        policy_observation_digest: "sha256:test".to_owned(),
        policy_basis: "test".to_owned(),
        external_carrier_absent: true,
    };
    let mut value = protocol::network_value_for_test(&observation);
    assert_eq!(value["non_packet_socket_count"], json!(3));
    assert!(value.get("inet_socket_count").is_none());
    assert_eq!(protocol::parse_network_for_test(&value).unwrap(), observation);

    let count = value
        .as_object_mut()
        .unwrap()
        .remove("non_packet_socket_count")
        .unwrap();
    value["inet_socket_count"] = count;
    let error = protocol::parse_network_for_test(&value).unwrap_err();
    assert_eq!(error.code, "PREPARED_JSON_FIELDS");
}

#[test]
fn namespace_join_rejects_forged_identifier_map() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let mut config = fixture.config(&plan);
    config.outside_uid = 1000;
    config.outside_gid = 2000;
    let forged = complete_namespace("0 7 1", "0 2000 1");

    let error =
        parent::validate_namespaces_for_test(&forged, &config).unwrap_err();
    assert_eq!(error.code, "PREPARED_NAMESPACE_EVIDENCE");
}

#[cfg(unix)]
#[test]
fn process_failure_prioritizes_deadline_over_child_stderr() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let config = fixture.config(&plan);
    let mut process = successful_setup_process(&config);
    set_child_failure(&mut process);
    process.termination_reason = TerminationReason::Deadline;

    let error =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(error.code, "PREPARED_SETUP_DEADLINE");
}

#[cfg(unix)]
#[test]
fn process_failure_retains_redacted_child_json() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let config = fixture.config(&plan);
    let mut process = successful_setup_process(&config);
    set_child_failure(&mut process);

    let error =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(error.code, "PREPARED_SETUP_CHILD_FAILURE");
    assert!(error.message.contains("child_code=PREPARED_CHILD_TEST"));
    assert!(error.message.contains("<path>"));
    assert!(!error.message.contains("/sensitive/path"));
}

#[cfg(unix)]
#[test]
fn process_failure_distinguishes_stream_failures() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let config = fixture.config(&plan);
    let mut process = successful_setup_process(&config);
    process.stdout_bytes += 1;
    let accounting =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(accounting.code, "PREPARED_SETUP_STREAM_ACCOUNTING");

    let mut process = successful_setup_process(&config);
    process.stdout_overflow = true;
    let overflow =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(overflow.code, "PREPARED_SETUP_STREAM_LIMIT");

    let mut process = successful_setup_process(&config);
    set_child_failure(&mut process);
    process.stderr_overflow = true;
    let partial_child =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(partial_child.code, "PREPARED_SETUP_STREAM_LIMIT");

    let mut process = successful_setup_process(&config);
    process.stdout_read_error = Some("read /sensitive/path failed".to_owned());
    let read =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(read.code, "PREPARED_SETUP_STREAM_READ");
    assert!(!read.message.contains("/sensitive/path"));
}

#[cfg(unix)]
#[test]
fn process_failure_distinguishes_ownership_facts() {
    let fixture = Fixture::new();
    let plan = fixture.plan();
    let config = fixture.config(&plan);
    let mut process = successful_setup_process(&config);
    process.group_identity_validated = false;
    let group =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(group.code, "PREPARED_SETUP_PROCESS_GROUP");

    let mut process = successful_setup_process(&config);
    process.descendants_reaped = false;
    let descendants =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(descendants.code, "PREPARED_SETUP_DESCENDANTS");

    let mut process = successful_setup_process(&config);
    process.reaped = false;
    let leader =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(leader.code, "PREPARED_SETUP_LEADER_REAP");

    let mut process = successful_setup_process(&config);
    process.readers_joined = false;
    let readers =
        parent::validate_process_for_test(&process, &config).unwrap_err();
    assert_eq!(readers.code, "PREPARED_SETUP_READERS");
}

#[test]
fn mountpoint_validation_failure_removes_created_directory() {
    let fixture = Fixture::new();
    let path = fixture.mount_parent.join("invalid-owner");
    let (uid, _) = linux::current_ids().unwrap();

    let error = linux::create_mountpoint_candidate_for_test(
        path.clone(),
        uid.wrapping_add(1),
    )
    .unwrap_err();

    assert_eq!(error.code, "PREPARED_MOUNTPOINT_IDENTITY");
    let failure = fs::symlink_metadata(path).unwrap_err();
    assert_eq!(failure.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn mountpoint_creation_guard_cleans_without_following_aliases() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let owned = fixture.mount_parent.join("owned");
    let target = fixture.mount_parent.join("alias-target");
    let alias = fixture.mount_parent.join("alias");
    fs::create_dir(&owned).unwrap();
    fs::create_dir(&target).unwrap();
    symlink(&target, &alias).unwrap();

    linux::cleanup_created_mountpoint_for_test(owned.clone());
    linux::cleanup_created_mountpoint_for_test(alias.clone());

    let failure = fs::symlink_metadata(owned).unwrap_err();
    assert_eq!(failure.kind(), std::io::ErrorKind::NotFound);
    assert!(fs::symlink_metadata(alias).unwrap().file_type().is_symlink());
    assert!(target.is_dir());
}

#[test]
fn mount_cleanup_retains_failed_unmount_for_drop_retry() {
    let fixture = Fixture::new();
    let path = fixture.mount_parent.join("not-a-mount");
    fs::create_dir(&path).unwrap();
    let mut mounts = linux::MountStack::default();
    mounts.push(path.clone(), linux::MountKind::Exposure);

    let cleanup = mounts.cleanup();

    let error = cleanup.result.unwrap_err();
    assert_eq!(error.code, "PREPARED_UNMOUNT");
    assert!(!cleanup.exposures_unmounted);
    assert_eq!(mounts.pending_mounts_for_test(), 1);
    drop(mounts);
    assert!(path.is_dir());
}

#[test]
fn inventory_retains_zero_byte_file() {
    let fixture = Fixture::new();
    let source = fixture.base.join("zero");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("empty"), []).unwrap();
    let source = source.canonicalize().unwrap();
    let limits =
        InventoryLimits { path_bytes: 4096, entry_count: 8, total_bytes: 8 };

    let observed =
        inventory::observe(&source, ExposureKind::Directory, limits).unwrap();
    assert_eq!(observed.entry_count, 2);
    assert_eq!(observed.total_bytes, 0);
    assert_ne!(observed.aggregate_digest, "");
}

#[test]
fn inventory_rejects_symlink() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let source = fixture.base.join("linked");
    fs::create_dir(&source).unwrap();
    symlink(&fixture.checker, source.join("alias")).unwrap();
    let source = source.canonicalize().unwrap();
    let limits =
        InventoryLimits { path_bytes: 4096, entry_count: 8, total_bytes: 8 };

    let error = inventory::observe(&source, ExposureKind::Directory, limits)
        .unwrap_err();
    assert_eq!(error.code, "PREPARED_INVENTORY_PATH_ESCAPE");
}

#[test]
fn netfilter_parser_accepts_only_done_dump() {
    let done = netlink_message(3, 2, 1, 7, &[]);

    assert!(network::parse_policy_for_test(&done).unwrap());
}

#[test]
fn netfilter_parser_rejects_table_and_wrong_sequence() {
    let table = netlink_message(10 << 8, 2, 1, 7, &[2, 0, 0, 0]);
    let error = network::parse_policy_for_test(&table).unwrap_err();
    assert_eq!(error.code, "PREPARED_NETWORK_POLICY_NONEMPTY");

    let wrong_sequence = netlink_message(3, 2, 2, 7, &[]);
    let error = network::parse_policy_for_test(&wrong_sequence).unwrap_err();
    assert_eq!(error.code, "PREPARED_NETWORK_POLICY_IDENTITY");

    let wrong_port = netlink_message(3, 2, 1, 8, &[]);
    let error = network::parse_policy_for_test(&wrong_port).unwrap_err();
    assert_eq!(error.code, "PREPARED_NETWORK_POLICY_IDENTITY");
}

#[test]
fn netfilter_parser_rejects_truncated_header() {
    let error = network::parse_policy_for_test(&[16, 0, 0]).unwrap_err();
    assert_eq!(error.code, "PREPARED_NETWORK_POLICY_FRAMING");
}

#[test]
fn netfilter_request_is_exact_and_deterministic() {
    let first = network::policy_request_for_test();
    let second = network::policy_request_for_test();

    assert_eq!(first, second);
    assert_eq!(u32::from_ne_bytes(first[0..4].try_into().unwrap()), 20);
    assert_eq!(u32::from_ne_bytes(first[8..12].try_into().unwrap()), 1);
}

#[test]
fn socket_observation_counts_every_claimed_proc_table() {
    let bounds = Fixture::new().plan().dispatch_bounds;
    let non_packet_tables = network::non_packet_socket_tables_for_test();
    let packet_tables = network::packet_socket_tables_for_test();
    assert_eq!(
        non_packet_tables,
        [
            "/proc/net/tcp",
            "/proc/net/tcp6",
            "/proc/net/udp",
            "/proc/net/udp6",
            "/proc/net/raw",
            "/proc/net/raw6",
            "/proc/net/icmp",
            "/proc/net/icmp6",
            "/proc/net/unix",
            "/proc/net/netlink",
        ]
    );
    assert_eq!(packet_tables, ["/proc/net/packet"]);

    let counts = network::socket_counts_for_test(bounds).unwrap();
    assert_eq!(counts, (non_packet_tables.len(), packet_tables.len()));

    let mut row_bound = bounds;
    row_bound.json_items = non_packet_tables.len();
    let error = network::socket_counts_for_test(row_bound).unwrap_err();
    assert_eq!(error.code, "PREPARED_NETWORK_SOCKET_LIMIT");

    let mut byte_bound = bounds;
    byte_bound.result_bytes = 128;
    let error = network::socket_counts_for_test(byte_bound).unwrap_err();
    assert_eq!(error.code, "PREPARED_NETWORK_SOCKET_BYTE_LIMIT");
}

#[test]
fn socket_observation_rejects_malformed_and_excess_rows() {
    let missing_header =
        network::socket_rows_for_test("/proc/net/unix", b"", 1).unwrap_err();
    assert_eq!(missing_header.code, "PREPARED_NETWORK_SOCKET_FORMAT");

    let invalid_utf8 =
        network::socket_rows_for_test("/proc/net/netlink", &[0xff], 1)
            .unwrap_err();
    assert_eq!(invalid_utf8.code, "PREPARED_NETWORK_SOCKET_UTF8");

    let excess = network::socket_rows_for_test(
        "/proc/net/packet",
        b"header\none\ntwo\n",
        1,
    )
    .unwrap_err();
    assert_eq!(excess.code, "PREPARED_NETWORK_SOCKET_LIMIT");
}

#[test]
fn netlink_socket_rows_distinguish_kernel_and_userspace_owners() {
    let header = "sk Eth Pid Groups Rmem Wmem Dump Locks Drops Inode\n";
    let kernel = format!("{header}0000000000000001 0 0 00000000 0 0 0 2 0 1\n");
    let userspace =
        format!("{header}0000000000000001 0 7 00000000 0 0 0 2 0 1\n");
    assert_eq!(
        network::socket_rows_for_test(
            "/proc/net/netlink",
            kernel.as_bytes(),
            1
        )
        .unwrap(),
        0
    );
    assert_eq!(
        network::socket_rows_for_test(
            "/proc/net/netlink",
            userspace.as_bytes(),
            1,
        )
        .unwrap(),
        1
    );
}

#[test]
fn ipv6_routes_distinguish_kernel_baseline_from_carrier_routes() {
    let bounds = Fixture::new().plan().dispatch_bounds;
    let kernel = concat!(
        "00000000000000000000000000000000 00 ",
        "00000000000000000000000000000000 00 ",
        "00000000000000000000000000000000 ",
        "ffffffff 00000001 00000000 00200200 lo\n",
    );
    let baseline = format!("{kernel}{kernel}");
    assert_eq!(
        network::ipv6_routes_for_test(baseline.as_bytes(), bounds).unwrap(),
        (0, 2)
    );

    let carrier = kernel.replace("00200200", "00000001");
    assert_eq!(
        network::ipv6_routes_for_test(carrier.as_bytes(), bounds).unwrap(),
        (1, 0)
    );
}

#[test]
fn netlink_socket_rows_reject_malformed_or_extra_columns() {
    let header = "sk Eth Pid Groups Rmem Wmem Dump Locks Drops Inode\n";
    let malformed =
        format!("{header}0000000000000001 0 nope 00000000 0 0 0 2 0 1\n");
    let extra =
        format!("{header}0000000000000001 0 7 00000000 0 0 0 2 0 1 extra\n");
    for mutant in [malformed, extra] {
        let error = network::socket_rows_for_test(
            "/proc/net/netlink",
            mutant.as_bytes(),
            1,
        )
        .unwrap_err();
        assert_eq!(error.code, "PREPARED_NETWORK_SOCKET_FORMAT");
    }
}

#[test]
fn netlink_nonblocking_flags_preserve_existing_status() {
    use nix::fcntl::OFlag;

    let current = OFlag::O_APPEND;
    let flags = network::nonblocking_flags_for_test(current.bits());

    assert!(flags.contains(OFlag::O_APPEND));
    assert!(flags.contains(OFlag::O_NONBLOCK));
}

#[test]
fn final_line_rejects_extra_output() {
    let error =
        linux::one_final_line(b"{}\n{}\n", "TEST_FINAL_LINE").unwrap_err();
    assert_eq!(error.code, "TEST_FINAL_LINE");
}

#[test]
fn pid1_ready_capture_accepts_partial_and_complete_prefixes() {
    let ready = protocol::ready_frame();
    assert!(!setup::ready_prefix_for_test(&ready[..8]).unwrap());
    assert!(setup::ready_prefix_for_test(ready).unwrap());

    let mut ready_and_result = ready.to_vec();
    ready_and_result.extend_from_slice(b"{}\n");
    assert!(setup::ready_prefix_for_test(&ready_and_result).unwrap());

    let mut malformed = ready.to_vec();
    malformed[1] = b'!';
    let error = setup::ready_prefix_for_test(&malformed).unwrap_err();
    assert_eq!(error.code, "PREPARED_PID1_HANDSHAKE");
}

#[test]
fn pid1_pipe_capture_enforces_limit_and_eof() {
    let mut retained = Vec::new();
    setup::append_bounded_for_test(&mut retained, b"1234", 4).unwrap();
    let error =
        setup::append_bounded_for_test(&mut retained, b"5", 4).unwrap_err();
    assert_eq!(error.code, "PREPARED_READER_LIMIT");

    let mut reader = std::io::Cursor::new(b"ok".as_slice());
    let mut captured = Vec::new();
    let mut eof = false;
    assert!(
        setup::read_once_for_test(&mut reader, &mut captured, &mut eof, 2)
            .unwrap()
    );
    assert_eq!(captured, b"ok");
    assert!(
        setup::read_once_for_test(&mut reader, &mut captured, &mut eof, 2)
            .unwrap()
    );
    assert!(eof);
    assert!(
        !setup::read_once_for_test(&mut reader, &mut captured, &mut eof, 2)
            .unwrap()
    );

    let mut pending = PendingReader;
    let mut pending_bytes = Vec::new();
    let mut pending_eof = false;
    assert!(
        !setup::read_once_for_test(
            &mut pending,
            &mut pending_bytes,
            &mut pending_eof,
            2,
        )
        .unwrap()
    );
    assert!(!pending_eof);
}

#[cfg(unix)]
#[test]
fn pid1_status_retains_typed_redacted_child_diagnostic() {
    use std::os::unix::process::ExitStatusExt as _;

    let fixture = Fixture::new();
    let bounds = fixture.plan().dispatch_bounds;
    let status = std::process::ExitStatus::from_raw(70 << 8);
    let stderr = concat!(
        "{\"code\":\"PREPARED_PID1_TEST\",",
        "\"message\":\"failed at /sensitive/path input\"}\n",
    )
    .as_bytes();

    let error = setup::validate_status_for_test(status, false, stderr, bounds)
        .unwrap_err();
    assert_eq!(error.code, "PREPARED_PID1_STATUS");
    assert!(error.message.contains("child_code=PREPARED_PID1_TEST"));
    assert!(error.message.contains("<path>"));
    assert!(!error.message.contains("/sensitive/path"));
}

#[cfg(unix)]
#[test]
fn pid1_status_digests_malformed_child_diagnostic() {
    use std::os::unix::process::ExitStatusExt as _;

    let fixture = Fixture::new();
    let bounds = fixture.plan().dispatch_bounds;
    let status = std::process::ExitStatus::from_raw(70 << 8);
    let stderr = b"malformed /sensitive/path\n";

    let error = setup::validate_status_for_test(status, false, stderr, bounds)
        .unwrap_err();
    assert_eq!(error.code, "PREPARED_PID1_STATUS");
    assert!(error.message.contains("malformed_child_diagnostic"));
    assert!(error.message.contains("digest=sha256:"));
    assert!(!error.message.contains("/sensitive/path"));
}

#[test]
#[ignore = "explicitly requested live prepared-root capability check"]
fn live_capability_probe_is_explicit() {
    let _ = super::capability_probe();
}

fn netlink_message(
    kind: u16,
    flags: u16,
    sequence: u32,
    sender: u32,
    payload: &[u8],
) -> Vec<u8> {
    let length = 16_usize.checked_add(payload.len()).unwrap();
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(&u32::try_from(length).unwrap().to_ne_bytes());
    bytes.extend_from_slice(&kind.to_ne_bytes());
    bytes.extend_from_slice(&flags.to_ne_bytes());
    bytes.extend_from_slice(&sequence.to_ne_bytes());
    bytes.extend_from_slice(&sender.to_ne_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn expanded_bootstrap(selected: DispatchBounds) -> DispatchBounds {
    DispatchBounds {
        config_bytes: selected.config_bytes * 2,
        result_bytes: selected.result_bytes * 2,
        json_depth: selected.json_depth * 2,
        json_items: selected.json_items * 2,
        execution_deadline: selected.execution_deadline * 2,
        supervision_deadline: selected.supervision_deadline * 2,
    }
}

fn complete_namespace(uid_map: &str, gid_map: &str) -> NamespaceObservation {
    let mut value =
        NamespaceObservation::empty(uid_map.to_owned(), gid_map.to_owned());
    for fact in [
        NamespaceFact::UserPrivate,
        NamespaceFact::MountPrivate,
        NamespaceFact::NetworkPrivate,
        NamespaceFact::PidPrivate,
        NamespaceFact::UidMapSingle,
        NamespaceFact::GidMapSingle,
        NamespaceFact::SetgroupsDenied,
        NamespaceFact::PrivatePropagation,
        NamespaceFact::PidOne,
        NamespaceFact::ParentDeathSignal,
    ] {
        value.record(fact, true);
    }
    value
}

#[cfg(unix)]
fn successful_setup_process(config: &Config) -> ProcessResult {
    use std::os::unix::process::ExitStatusExt as _;

    ProcessResult {
        argv: vec![
            config.checker_executable.as_os_str().to_owned(),
            OsString::from(super::SETUP_ARGUMENT),
        ],
        status: std::process::ExitStatus::from_raw(0),
        stdout: b"{}\n".to_vec(),
        stderr: Vec::new(),
        stdout_bytes: 3,
        stderr_bytes: 0,
        stdout_overflow: false,
        stderr_overflow: false,
        stdout_read_error: None,
        stderr_read_error: None,
        termination_reason: TerminationReason::Exited,
        kill_attempted: false,
        kill_raced_with_exit: false,
        limit_observed_after_exit: false,
        leader_pid: 10,
        pgid: Some(10),
        group_identity_validated: true,
        group_absent_after_reap: true,
        descendants_owned: true,
        descendants_reaped: true,
        descendant_residue_observed: false,
        reaped: true,
        readers_joined: true,
    }
}

#[cfg(unix)]
fn set_child_failure(process: &mut ProcessResult) {
    use std::os::unix::process::ExitStatusExt as _;

    process.status = std::process::ExitStatus::from_raw(70 << 8);
    process.stderr = concat!(
        "{\"code\":\"PREPARED_CHILD_TEST\",",
        "\"message\":\"failed at /sensitive/path input\"}\n",
    )
    .as_bytes()
    .to_vec();
    process.stderr_bytes = u64::try_from(process.stderr.len()).unwrap();
}

struct Fixture {
    base: PathBuf,
    repository: PathBuf,
    mount_parent: PathBuf,
    checker: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "reovim-prepared-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&base).unwrap();
        let repository = base.join("repository");
        let mount_parent = base.join("mounts");
        fs::create_dir(&repository).unwrap();
        fs::write(repository.join("input"), b"input\n").unwrap();
        fs::create_dir(&mount_parent).unwrap();
        let checker = base.join("checker");
        fs::write(&checker, b"checker\n").unwrap();
        fs::set_permissions(&checker, fs::Permissions::from_mode(0o700))
            .unwrap();
        Self {
            base: base.canonicalize().unwrap(),
            repository: repository.canonicalize().unwrap(),
            mount_parent: mount_parent.canonicalize().unwrap(),
            checker: checker.canonicalize().unwrap(),
        }
    }

    fn plan(&self) -> Plan {
        Plan {
            repository_root: self.repository.clone(),
            mount_parent: self.mount_parent.clone(),
            checker_executable: self.checker.clone(),
            execution_environment: "linux-test".to_owned(),
            workload_request: json!({
                "schema": "reovim-test-workload/v1",
                "limit": 7,
            }),
            exposures: vec![
                ExposurePlan {
                    id: "checker".to_owned(),
                    source: self.checker.clone(),
                    destination: PathBuf::from("/tool/checker"),
                    kind: ExposureKind::File,
                    executable: true,
                    purpose: "Product executable".to_owned(),
                },
                ExposurePlan {
                    id: "file:/repo/input".to_owned(),
                    source: self.repository.join("input"),
                    destination: PathBuf::from("/repo/input"),
                    kind: ExposureKind::File,
                    executable: false,
                    purpose: "Product repository input".to_owned(),
                },
                ExposurePlan {
                    id: "null-device".to_owned(),
                    source: PathBuf::from("/dev/null"),
                    destination: PathBuf::from("/dev/null"),
                    kind: ExposureKind::CharacterDevice,
                    executable: false,
                    purpose: "null standard-input device".to_owned(),
                },
            ],
            inventory_limits: InventoryLimits {
                path_bytes: 4096,
                entry_count: 64,
                total_bytes: 1024 * 1024,
            },
            scratch_limits: ScratchLimits {
                size_bytes: 4 * 1024 * 1024,
                inode_count: 1024,
            },
            resource_limits: ResourceLimits {
                address_space_bytes: 128 * 1024 * 1024,
                cpu_seconds: 10,
                open_files: 64,
                output_file_bytes: 4 * 1024 * 1024,
                core_dump_bytes: 0,
            },
            dispatch_bounds: DispatchBounds {
                config_bytes: 256 * 1024,
                result_bytes: 256 * 1024,
                json_depth: 32,
                json_items: 4096,
                execution_deadline: Duration::from_secs(5),
                supervision_deadline: Duration::from_secs(10),
            },
            workload_result_bytes: 64 * 1024,
            descendant_reap_limit: 64,
        }
    }

    fn config(&self, plan: &Plan) -> Config {
        let exposures = plan
            .exposures
            .iter()
            .map(|exposure| {
                let expected = inventory::observe(
                    &exposure.source,
                    exposure.kind,
                    plan.inventory_limits,
                )
                .unwrap();
                ConfiguredExposure { plan: exposure.clone(), expected }
            })
            .collect();
        Config {
            mountpoint: self.mount_parent.join("prepared"),
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
            parent_namespaces: NamespaceLinks {
                user: "user:[1]".to_owned(),
                mount: "mnt:[2]".to_owned(),
                network: "net:[3]".to_owned(),
                pid: "pid:[4]".to_owned(),
            },
            outside_uid: 1000,
            outside_gid: 1000,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

struct PendingReader;

impl std::io::Read for PendingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
    }
}
