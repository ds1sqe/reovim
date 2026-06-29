//! Selftests for the bounded dump snapshot header.

use {
    super::{
        DumpParseError, DumpSink, clear_sink_for_tests, encode_snapshot_artifact,
        encode_snapshot_header, install_sink, parse_snapshot_header, parse_snapshot_header_prefix,
        record_panic_state, reset_panic_record_for_tests, status, sync,
    },
    crate::{klog, mm, proc, program, service, syscall},
    core::{
        cell::UnsafeCell,
        sync::atomic::{AtomicBool, AtomicUsize},
    },
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_panic::{Disposition, PanicRecord},
};

struct SinkBuffer {
    bytes: UnsafeCell<[u8; super::MAX_DUMP_ARTIFACT_BYTES]>,
    len: AtomicUsize,
    flush_count: AtomicUsize,
    flush_ok: AtomicBool,
}

// SAFETY: selftests run single-threaded; atomics/UnsafeCell are used only to
// model the kernel-global sink interface without allocation.
unsafe impl Sync for SinkBuffer {}

static SINK_BUFFER: SinkBuffer = SinkBuffer {
    bytes: UnsafeCell::new([0u8; super::MAX_DUMP_ARTIFACT_BYTES]),
    len: AtomicUsize::new(0),
    flush_count: AtomicUsize::new(0),
    flush_ok: AtomicBool::new(true),
};

fn sink_clear() {
    SINK_BUFFER
        .len
        .store(0, core::sync::atomic::Ordering::Relaxed);
    SINK_BUFFER
        .flush_count
        .store(0, core::sync::atomic::Ordering::Relaxed);
    SINK_BUFFER
        .flush_ok
        .store(true, core::sync::atomic::Ordering::Relaxed);
    // SAFETY: selftest execution is single-threaded.
    unsafe {
        (*SINK_BUFFER.bytes.get()).fill(0);
    }
}

fn sink_write(offset: usize, bytes: &[u8]) -> bool {
    if offset != 0 {
        return false;
    }
    if bytes.len() > super::MAX_DUMP_ARTIFACT_BYTES {
        return false;
    }
    // SAFETY: selftest execution is single-threaded.
    unsafe {
        let out = &mut *SINK_BUFFER.bytes.get();
        out[..bytes.len()].copy_from_slice(bytes);
    }
    SINK_BUFFER
        .len
        .store(bytes.len(), core::sync::atomic::Ordering::Relaxed);
    true
}

fn sink_flush() -> bool {
    SINK_BUFFER
        .flush_count
        .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    SINK_BUFFER
        .flush_ok
        .load(core::sync::atomic::Ordering::Relaxed)
}

fn sink_read_back(offset: usize, out: &mut [u8]) -> usize {
    if offset != 0 {
        return 0;
    }
    let len = SINK_BUFFER.len.load(core::sync::atomic::Ordering::Relaxed);
    let copy_len = core::cmp::min(len, out.len());
    // SAFETY: selftest execution is single-threaded.
    unsafe {
        let bytes = &*SINK_BUFFER.bytes.get();
        out[..copy_len].copy_from_slice(&bytes[..copy_len]);
    }
    copy_len
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let mut index = 0usize;
    while index + needle.len() <= haystack.len() {
        let mut matched = 0usize;
        while matched < needle.len() && haystack[index + matched] == needle[matched] {
            matched += 1;
        }
        if matched == needle.len() {
            return Some(index);
        }
        index += 1;
    }
    None
}

arch_test!(dump_snapshot_header_round_trips_without_alloc, {
    clear_sink_for_tests();
    klog::reset();
    proc::reset();
    syscall::reset();
    klog::append_event("dump", "info", "selftest");

    let header = encode_snapshot_header().expect("dump header encodes");
    let bytes = header.as_bytes();
    testrt::check(find_bytes(bytes, b"reovim-dump-v1\n").is_some(), "header names dump format");
    testrt::check(find_bytes(bytes, b"package=unknown\n").is_some(), "header includes package");
    testrt::check(find_bytes(bytes, b"target=unknown\n").is_some(), "header includes target");
    testrt::check(
        find_bytes(bytes, b"boot_memory_ranges=0\n").is_some(),
        "header includes boot memory ranges",
    );
    testrt::check(
        find_bytes(bytes, b"device_records=0\n").is_some(),
        "header includes device inventory count",
    );
    testrt::check(
        find_bytes(bytes, b"proof_state=operator-required\n").is_some(),
        "header includes proof state",
    );
    testrt::check(
        find_bytes(bytes, b"panic_state=none\n").is_some(),
        "header includes panic state",
    );
    testrt::check(
        find_bytes(bytes, b"last_sync_attempted=false\n").is_some(),
        "header includes last sync attempted flag",
    );
    testrt::check(
        find_bytes(bytes, b"last_sync_reason=never-synced\n").is_some(),
        "header includes last sync reason",
    );
    testrt::check(find_bytes(bytes, b"checksum=").is_some(), "header includes checksum");

    let parsed = parse_snapshot_header(bytes).expect("dump header parses");
    testrt::check_eq(parsed.format_version, 1usize);
    testrt::check_eq(parsed.boot_id, 1usize);
    testrt::check_eq(parsed.session_id, 1usize);
    testrt::check_eq(parsed.identity_source, "volatile-memory");
    testrt::check_eq(parsed.package, "unknown");
    testrt::check_eq(parsed.version, "unknown");
    testrt::check_eq(parsed.target, "unknown");
    testrt::check_eq(parsed.selected_profile, "unknown");
    testrt::check_eq(parsed.profile_request, "unknown");
    testrt::check_eq(parsed.bootline, "unknown");
    testrt::check_eq(parsed.launch_profile_feature, "unknown");
    testrt::check_eq(parsed.boot_memory_ranges, 0usize);
    testrt::check_eq(parsed.boot_memory_usable_bytes, 0usize);
    testrt::check_eq(parsed.boot_cpu_count, 0usize);
    testrt::check_eq(parsed.boot_heap_total_bytes, 0usize);
    testrt::check_eq(parsed.device_records, 0usize);
    testrt::check_eq(parsed.proof_state, "operator-required");
    testrt::check_eq(parsed.panic_state, "none");
    testrt::check_eq(parsed.panic_records, 0usize);
    testrt::check_eq(parsed.persistent_available, false);
    testrt::check_eq(parsed.storage, "none");
    testrt::check_eq(parsed.storage_capacity_bytes, 0usize);
    testrt::check_eq(parsed.last_sync_attempted, false);
    testrt::check_eq(parsed.last_sync_persistent_available, false);
    testrt::check_eq(parsed.last_sync_storage, "none");
    testrt::check_eq(parsed.last_sync_storage_capacity_bytes, 0usize);
    testrt::check_eq(parsed.last_sync_written, false);
    testrt::check_eq(parsed.last_sync_bytes_written, 0usize);
    testrt::check_eq(parsed.last_sync_checksum, 0u32);
    testrt::check_eq(parsed.last_sync_verified, false);
    testrt::check_eq(parsed.last_sync_reason, "never-synced");
    testrt::check_eq(parsed.event_records, 1usize);
    testrt::check_eq(parsed.process_records, 2usize);
    testrt::check_eq(parsed.service_records, 0usize);
    testrt::check_eq(parsed.exec_load_records, 0usize);
    testrt::check_eq(parsed.pending_exec_records, 0usize);
    testrt::check_eq(parsed.task_records, 2usize);
    testrt::check(parsed.checksum != 0, "checksum is populated");
});

arch_test!(dump_artifact_includes_recorded_panic_state, {
    clear_sink_for_tests();
    reset_panic_record_for_tests();
    klog::reset();
    proc::reset();
    syscall::reset();

    record_panic_state(PanicRecord {
        disposition: Disposition::Halt,
        rollback_failed: true,
    });

    let status = status();
    testrt::check_eq(status.panic_state, "recorded");
    testrt::check_eq(status.panic_records, 1usize);
    let artifact = encode_snapshot_artifact().expect("dump artifact encodes");
    let bytes = artifact.as_bytes();
    testrt::check(
        find_bytes(bytes, b"panic_state=recorded\n").is_some(),
        "header records panic state",
    );
    testrt::check(find_bytes(bytes, b"panic_records=1\n").is_some(), "header records panic count");
    testrt::check(
        find_bytes(bytes, b"panic:\nstate=recorded\nrecords=1\n").is_some(),
        "artifact has recorded panic section",
    );
    testrt::check(
        find_bytes(bytes, b"- disposition=halt rollback_failed=true\n").is_some(),
        "artifact has panic record detail",
    );

    let parsed = parse_snapshot_header_prefix(bytes).expect("artifact header prefix parses");
    testrt::check_eq(parsed.panic_state, "recorded");
    testrt::check_eq(parsed.panic_records, 1usize);

    reset_panic_record_for_tests();
});

arch_test!(dump_artifact_includes_retained_tables, {
    clear_sink_for_tests();
    klog::reset();
    proc::reset();
    syscall::reset();
    service::request_service(3, 3, "shell", "/bin/sh").expect("service record");
    service::mark_started("shell", 2, 2);
    let mut argv = program::ProgramArgvBuffer::empty();
    testrt::check(argv.push("blocked").is_ok(), "dump process argv0 accepted");
    testrt::check(argv.push("/boot").is_ok(), "dump process argv1 accepted");
    let mut env = program::ProgramEnvBuffer::empty();
    testrt::check(
        env.push("REOVIM_MEDIA_PATH", "/boot/status").is_ok(),
        "dump process env accepted",
    );
    let child = proc::spawn_child_with_program_argv_env_mapped(
        proc::ROOTD_PID,
        proc::ROOTD_PID,
        "/payload/blocked",
        "selftest-loader",
        "payload_blocked",
        mm::AddressSpaceImage::metadata_only("/payload/blocked"),
        &argv,
        &env,
        "blocked",
        crate::exec_artifact::ExecArtifactBodyFormat::None,
        crate::exec_artifact::ExecArtifactBodyInnerFormat::None,
    )
    .expect("dump selftest child process slot available");
    let _ = proc::block_process(child.pid);
    klog::append_event("dump", "info", "artifact-selftest");

    let artifact = encode_snapshot_artifact().expect("dump artifact encodes");
    let bytes = artifact.as_bytes();
    testrt::check(find_bytes(bytes, b"reovim-dump-v1\n").is_some(), "artifact has header");
    testrt::check(find_bytes(bytes, b"boot:\n").is_some(), "artifact has boot table");
    testrt::check(find_bytes(bytes, b"devices:\n").is_some(), "artifact has device table");
    testrt::check(find_bytes(bytes, b"proof:\n").is_some(), "artifact has proof table");
    testrt::check(find_bytes(bytes, b"dump-sync:\n").is_some(), "artifact has dump sync table");
    testrt::check(find_bytes(bytes, b"panic:\n").is_some(), "artifact has panic table");
    testrt::check(find_bytes(bytes, b"events:\n").is_some(), "artifact has events table");
    testrt::check(find_bytes(bytes, b"processes:\n").is_some(), "artifact has process table");
    testrt::check(find_bytes(bytes, b"services:\n").is_some(), "artifact has service table");
    testrt::check(
        find_bytes(
            bytes,
            b"name=shell target=/bin/sh state=started reason=running owner_pid=3 owner_task=3 service_pid=2 service_task=2",
        )
        .is_some(),
        "artifact retains init service handoff",
    );
    testrt::check(find_bytes(bytes, b"execs:\n").is_some(), "artifact has exec load table");
    testrt::check(find_bytes(bytes, b"pending:\n").is_some(), "artifact has pending exec table");
    testrt::check(find_bytes(bytes, b"scheduler:\n").is_some(), "artifact has scheduler state");
    testrt::check(
        find_bytes(bytes, b"current_pid=").is_some(),
        "artifact correlates scheduler current task with process id",
    );
    testrt::check(
        find_bytes(bytes, b"next_ready_pid=").is_some(),
        "artifact correlates scheduler ready queue with process ids",
    );
    testrt::check(find_bytes(bytes, b"tick_count=0\n").is_some(), "artifact has scheduler ticks");
    testrt::check(find_bytes(bytes, b"syscalls:\n").is_some(), "artifact has syscall table");
    testrt::check(
        find_bytes(bytes, b"continuations:\n").is_some(),
        "artifact has syscall continuation table",
    );
    testrt::check(find_bytes(bytes, b"tasks:\n").is_some(), "artifact has task table");
    testrt::check(find_bytes(bytes, b"waits:\n").is_some(), "artifact has wait table");
    testrt::check(
        find_bytes(
            bytes,
            b"state=blocked path=/payload/blocked exit=0 loader=selftest-loader entry_fn=payload_blocked block=operator",
        )
        .is_some(),
        "artifact retains process block reason",
    );
    testrt::check(
        find_bytes(
            bytes,
            b"argc=2 argv0=blocked argv0_truncated=false argv1=/boot argv1_truncated=false",
        )
        .is_some(),
        "artifact retains process argv metadata",
    );
    testrt::check(
        find_bytes(
            bytes,
            b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
        )
        .is_some(),
        "artifact retains process env metadata",
    );
    testrt::check(
        find_bytes(bytes, b" address_space=").is_some(),
        "artifact retains process address-space identity",
    );
    testrt::check(
        find_bytes(bytes, b" address_space_state=active").is_some(),
        "artifact correlates process address-space state",
    );
    testrt::check(
        find_bytes(bytes, b" address_space_source=/payload/blocked").is_some(),
        "artifact correlates process address-space source",
    );
    testrt::check(
        find_bytes(bytes, b" address_space_stack_bytes=").is_some(),
        "artifact correlates process stack reservation",
    );
    testrt::check(
        find_bytes(bytes, b" address_space_regions=1").is_some(),
        "artifact correlates process address-space region count",
    );
    testrt::check(
        find_bytes(
            bytes,
            b" address_space_page_table=3 address_space_mapped_pages=4 address_space_text_pages=0 address_space_stack_pages=4",
        )
        .is_some(),
        "artifact correlates process page-table page counts",
    );
    testrt::check(
        find_bytes(bytes, b" address_space_text_start=0 address_space_text_end=0").is_some(),
        "artifact correlates absent text region for metadata-only image",
    );
    testrt::check(
        find_bytes(bytes, b" address_space_stack_start=2147467264").is_some(),
        "artifact correlates process stack start",
    );
    testrt::check(
        find_bytes(bytes, b" address_space_stack_end=2147483648").is_some(),
        "artifact correlates process stack end",
    );
    testrt::check(
        find_bytes(bytes, b" address_space_stack_flags=27").is_some(),
        "artifact correlates process stack permissions",
    );
    testrt::check(
        find_bytes(bytes, b" artifact_body_format=none artifact_body_inner=none").is_some(),
        "artifact retains process executable body metadata",
    );
    testrt::check(
        find_bytes(bytes, b" image_generation=").is_some(),
        "artifact retains process image generation",
    );
    testrt::check(
        find_bytes(bytes, b"address-spaces:\n").is_some(),
        "artifact has address-space table",
    );
    testrt::check(
        find_bytes(bytes, b"owner_pid=3 image_generation=1 state=active path=/payload/blocked")
            .is_some(),
        "artifact retains active address-space ownership",
    );
    testrt::check(
        find_bytes(
            bytes,
            b"source=/payload/blocked text_bytes=0 text_checksum=0 stack_bytes=16384 regions=1 text_start=0 text_end=0 text_flags=0 stack_start=2147467264 stack_end=2147483648 stack_flags=27",
        )
        .is_some(),
        "artifact retains active address-space image map",
    );
    testrt::check(
        find_bytes(bytes, b"page_table=3 mapped_pages=4 text_pages=0 stack_pages=4").is_some(),
        "artifact retains active address-space page counts",
    );
    testrt::check_eq(mm::USER_STACK_TOP - mm::DEFAULT_USER_STACK_BYTES, 2147467264usize);
    testrt::check_eq(mm::ADDRESS_SPACE_STACK_FLAGS, 27u32);
    testrt::check(
        find_bytes(bytes, b"address-space-page-tables:\n").is_some(),
        "artifact has address-space page-table table",
    );
    testrt::check(
        find_bytes(
            bytes,
            b"id=3 address_space=3 owner_pid=3 image_generation=1 state=active root_table=3 source=/payload/blocked mapped_pages=4 text_pages=0 stack_pages=4 text_flags=0 stack_flags=27",
        )
        .is_some(),
        "artifact retains active address-space page table",
    );
    testrt::check(
        find_bytes(bytes, b"address-space-pages:\n").is_some(),
        "artifact has address-space page-entry table",
    );
    testrt::check(
        find_bytes(
            bytes,
            b"address_space=3 page_table=3 owner_pid=3 image_generation=1 state=active kind=stack backing=zero-fill source=/payload/blocked page_index=0 virtual_start=2147467264 virtual_end=2147471360 bytes=4096 source_offset=0 flags=27",
        )
        .is_some(),
        "artifact retains first stack page mapping",
    );
    testrt::check(
        find_bytes(
            bytes,
            b"address_space=3 page_table=3 owner_pid=3 image_generation=1 state=active kind=stack backing=zero-fill source=/payload/blocked page_index=3",
        )
        .is_some(),
        "artifact retains final stack page mapping",
    );
    testrt::check(
        find_bytes(bytes, b"address-space-objects:\n").is_some(),
        "artifact has address-space object table",
    );
    testrt::check(
        find_bytes(
            bytes,
            b"address_space=3 owner_pid=3 image_generation=1 state=active kind=stack backing=zero-fill source=/payload/blocked start=2147467264 end=2147483648 bytes=16384 checksum=0 flags=27",
        )
        .is_some(),
        "artifact retains active stack memory object",
    );
    testrt::check(
        find_bytes(bytes, b"flags=27 page_count=4").is_some(),
        "artifact retains memory-object page count",
    );
    testrt::check(
        find_bytes(
            bytes,
            b"state=blocked entry=/payload/blocked block=operator wake_tick=0 runs=0 ticks=0",
        )
        .is_some(),
        "artifact retains task block reason and scheduler counters",
    );
    testrt::check(parse_snapshot_header(bytes).is_err(), "header-only parser stays strict");

    let parsed = parse_snapshot_header_prefix(bytes).expect("artifact header prefix parses");
    testrt::check_eq(parsed.device_records, 0usize);
    testrt::check_eq(parsed.proof_state, "operator-required");
    testrt::check_eq(parsed.panic_state, "none");
    testrt::check_eq(parsed.event_records, 1usize);
    testrt::check_eq(parsed.process_records, 3usize);
    testrt::check_eq(parsed.exec_load_records, 0usize);
    testrt::check_eq(parsed.pending_exec_records, 0usize);
    testrt::check_eq(parsed.task_records, 3usize);
});

arch_test!(dump_snapshot_header_rejects_corruption, {
    clear_sink_for_tests();
    klog::reset();
    proc::reset();
    syscall::reset();

    let header = encode_snapshot_header().expect("dump header encodes");
    let mut bytes = [0u8; super::MAX_SNAPSHOT_HEADER_BYTES];
    let len = header.len;
    bytes[..len].copy_from_slice(header.as_bytes());
    let boot = find_bytes(&bytes[..len], b"boot_id=1").expect("boot id row exists");
    bytes[boot + b"boot_id=".len()] = b'2';

    testrt::check_eq(parse_snapshot_header(&bytes[..len]), Err(DumpParseError::ChecksumMismatch));
});

arch_test!(dump_sync_writes_and_verifies_installed_sink, {
    clear_sink_for_tests();
    sink_clear();
    klog::reset();
    proc::reset();
    syscall::reset();
    klog::append_event("dump", "info", "sync-selftest");

    install_sink(DumpSink::new_with_flush(
        "selftest-dump-block0",
        super::MAX_DUMP_ARTIFACT_BYTES,
        sink_write,
        sink_read_back,
        sink_flush,
    ));
    let before_status = status();
    testrt::check_eq(before_status.persistent_available, true);
    testrt::check_eq(before_status.storage, "selftest-dump-block0");
    testrt::check_eq(before_status.storage_capacity_bytes, super::MAX_DUMP_ARTIFACT_BYTES);

    let result = sync();
    testrt::check_eq(result.attempted, true);
    testrt::check_eq(result.persistent_available, true);
    testrt::check_eq(result.storage, "selftest-dump-block0");
    testrt::check_eq(result.storage_capacity_bytes, super::MAX_DUMP_ARTIFACT_BYTES);
    testrt::check_eq(result.written, true);
    testrt::check_eq(result.verified, true);
    testrt::check_eq(result.reason, "written-readback-ok");
    testrt::check_eq(
        SINK_BUFFER
            .flush_count
            .load(core::sync::atomic::Ordering::Relaxed),
        2usize,
    );
    let header = encode_snapshot_header().expect("dump header encodes");
    testrt::check(
        result.bytes_written > header.len,
        "sync writes bounded artifact bytes beyond the header",
    );
    testrt::check(result.checksum != 0, "sync reports verified checksum");
    let after_status = status();
    testrt::check_eq(after_status.last_sync.attempted, true);
    testrt::check_eq(after_status.last_sync.storage, "selftest-dump-block0");
    testrt::check_eq(after_status.last_sync.written, true);
    testrt::check_eq(after_status.last_sync.verified, true);
    testrt::check_eq(after_status.last_sync.reason, "written-readback-ok");

    let len = SINK_BUFFER.len.load(core::sync::atomic::Ordering::Relaxed);
    let mut bytes = [0u8; super::MAX_DUMP_ARTIFACT_BYTES];
    let read_len = sink_read_back(0, &mut bytes);
    testrt::check_eq(read_len, len);
    testrt::check(
        find_bytes(&bytes[..read_len], b"events:\n").is_some(),
        "synced artifact has events",
    );
    testrt::check(
        find_bytes(&bytes[..read_len], b"proof:\n").is_some(),
        "synced artifact has proof state",
    );
    testrt::check(
        find_bytes(&bytes[..read_len], b"panic:\n").is_some(),
        "synced artifact has panic state",
    );
    testrt::check(
        find_bytes(&bytes[..read_len], b"syscalls:\n").is_some(),
        "synced artifact has syscalls",
    );
    testrt::check(
        find_bytes(&bytes[..read_len], b"continuations:\n").is_some(),
        "synced artifact has syscall continuations",
    );
    testrt::check(
        find_bytes(&bytes[..read_len], b"pending:\n").is_some(),
        "synced artifact has pending exec table",
    );
    testrt::check(
        find_bytes(&bytes[..read_len], b"dump-sync:\nattempted=true\n").is_some(),
        "synced artifact records finalized dump-sync attempt",
    );
    testrt::check(
        find_bytes(&bytes[..read_len], b"reason=written-readback-ok\n").is_some(),
        "synced artifact records finalized dump-sync success",
    );
    let parsed =
        parse_snapshot_header_prefix(&bytes[..read_len]).expect("synced artifact header parses");
    testrt::check_eq(parsed.persistent_available, true);
    testrt::check_eq(parsed.storage, "selftest-dump-block0");
    testrt::check_eq(parsed.storage_capacity_bytes, super::MAX_DUMP_ARTIFACT_BYTES);
    testrt::check_eq(parsed.last_sync_attempted, true);
    testrt::check_eq(parsed.last_sync_persistent_available, true);
    testrt::check_eq(parsed.last_sync_storage, "selftest-dump-block0");
    testrt::check_eq(parsed.last_sync_written, true);
    testrt::check_eq(parsed.last_sync_verified, true);
    testrt::check_eq(parsed.last_sync_reason, "written-readback-ok");
    testrt::check(parsed.last_sync_checksum != 0, "final artifact records first checksum");
    testrt::check_eq(parsed.checksum, result.checksum);

    clear_sink_for_tests();
});

arch_test!(dump_sync_fails_closed_when_sink_flush_is_unavailable, {
    clear_sink_for_tests();
    sink_clear();

    install_sink(DumpSink::new(
        "unflushed-dump-block0",
        super::MAX_DUMP_ARTIFACT_BYTES,
        sink_write,
        sink_read_back,
    ));

    let result = sync();
    testrt::check_eq(result.attempted, true);
    testrt::check_eq(result.persistent_available, true);
    testrt::check_eq(result.storage, "unflushed-dump-block0");
    testrt::check_eq(result.written, false);
    testrt::check_eq(result.verified, false);
    testrt::check_eq(result.reason, "flush-unavailable");
    let after_status = status();
    testrt::check_eq(after_status.last_sync.attempted, true);
    testrt::check_eq(after_status.last_sync.storage, "unflushed-dump-block0");
    testrt::check_eq(after_status.last_sync.written, false);
    testrt::check_eq(after_status.last_sync.reason, "flush-unavailable");
    testrt::check_eq(
        SINK_BUFFER
            .flush_count
            .load(core::sync::atomic::Ordering::Relaxed),
        0usize,
    );

    clear_sink_for_tests();
});

arch_test!(dump_sync_fails_closed_when_sink_flush_fails, {
    clear_sink_for_tests();
    sink_clear();
    SINK_BUFFER
        .flush_ok
        .store(false, core::sync::atomic::Ordering::Relaxed);

    install_sink(DumpSink::new_with_flush(
        "failing-flush-dump-block0",
        super::MAX_DUMP_ARTIFACT_BYTES,
        sink_write,
        sink_read_back,
        sink_flush,
    ));

    let result = sync();
    testrt::check_eq(result.attempted, true);
    testrt::check_eq(result.persistent_available, true);
    testrt::check_eq(result.storage, "failing-flush-dump-block0");
    testrt::check_eq(result.written, false);
    testrt::check_eq(result.verified, false);
    testrt::check_eq(result.reason, "flush-failed");
    let after_status = status();
    testrt::check_eq(after_status.last_sync.attempted, true);
    testrt::check_eq(after_status.last_sync.storage, "failing-flush-dump-block0");
    testrt::check_eq(after_status.last_sync.written, false);
    testrt::check_eq(after_status.last_sync.reason, "flush-failed");
    testrt::check_eq(
        SINK_BUFFER
            .flush_count
            .load(core::sync::atomic::Ordering::Relaxed),
        1usize,
    );

    clear_sink_for_tests();
});

arch_test!(dump_sync_fails_closed_without_sink, {
    clear_sink_for_tests();
    let result = sync();
    testrt::check_eq(result.attempted, true);
    testrt::check_eq(result.persistent_available, false);
    testrt::check_eq(result.storage, "none");
    testrt::check_eq(result.storage_capacity_bytes, 0usize);
    testrt::check_eq(result.written, false);
    testrt::check_eq(result.verified, false);
    testrt::check_eq(result.reason, "no-persistent-dump-sink");
    let after_status = status();
    testrt::check_eq(after_status.last_sync.attempted, true);
    testrt::check_eq(after_status.last_sync.storage, "none");
    testrt::check_eq(after_status.last_sync.reason, "no-persistent-dump-sink");
});
