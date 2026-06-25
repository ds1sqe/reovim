//! Selftests for the diagnostic block target.

use {
    super::{
        BlockDevice, clear_diagnostic_device_for_tests, clear_source_media_device_for_tests,
        diagnostic_status, install_diagnostic_device, install_source_media_device,
        read_diagnostic_artifact, read_source_media_artifact, read_source_media_artifact_at,
        source_media_status, write_diagnostic_artifact,
    },
    core::{cell::UnsafeCell, sync::atomic::AtomicUsize},
    reovim_testrt::{self as testrt, arch_test},
};

const TEST_BLOCK_CAPACITY: usize = 64;

struct BlockBuffer {
    bytes: UnsafeCell<[u8; TEST_BLOCK_CAPACITY]>,
    len: AtomicUsize,
    last_write_offset: AtomicUsize,
    last_read_offset: AtomicUsize,
}

// SAFETY: selftests run single-threaded; atomics/UnsafeCell model the
// kernel-global callback interface without allocation.
unsafe impl Sync for BlockBuffer {}

static BLOCK_BUFFER: BlockBuffer = BlockBuffer {
    bytes: UnsafeCell::new([0u8; TEST_BLOCK_CAPACITY]),
    len: AtomicUsize::new(0),
    last_write_offset: AtomicUsize::new(usize::MAX),
    last_read_offset: AtomicUsize::new(usize::MAX),
};

fn reset_buffer() {
    use core::sync::atomic::Ordering;

    BLOCK_BUFFER.len.store(0, Ordering::Relaxed);
    BLOCK_BUFFER
        .last_write_offset
        .store(usize::MAX, Ordering::Relaxed);
    BLOCK_BUFFER
        .last_read_offset
        .store(usize::MAX, Ordering::Relaxed);
    // SAFETY: selftest execution is single-threaded.
    unsafe {
        (*BLOCK_BUFFER.bytes.get()).fill(0);
    }
}

fn block_write(offset: usize, bytes: &[u8]) -> bool {
    use core::sync::atomic::Ordering;

    if offset != 0 || bytes.len() > TEST_BLOCK_CAPACITY {
        return false;
    }
    // SAFETY: selftest execution is single-threaded.
    unsafe {
        let out = &mut *BLOCK_BUFFER.bytes.get();
        out[..bytes.len()].copy_from_slice(bytes);
    }
    BLOCK_BUFFER
        .last_write_offset
        .store(offset, Ordering::Relaxed);
    BLOCK_BUFFER.len.store(bytes.len(), Ordering::Relaxed);
    true
}

fn block_read(offset: usize, out: &mut [u8]) -> usize {
    use core::sync::atomic::Ordering;

    BLOCK_BUFFER
        .last_read_offset
        .store(offset, Ordering::Relaxed);
    let len = BLOCK_BUFFER.len.load(Ordering::Relaxed);
    let copy_len = core::cmp::min(len, out.len());
    // SAFETY: selftest execution is single-threaded.
    unsafe {
        let bytes = &*BLOCK_BUFFER.bytes.get();
        out[..copy_len].copy_from_slice(&bytes[..copy_len]);
    }
    copy_len
}

arch_test!(diagnostic_block_target_writes_and_reads_artifact, {
    use core::sync::atomic::Ordering;

    clear_diagnostic_device_for_tests();
    reset_buffer();
    testrt::check(diagnostic_status().is_none(), "no diagnostic block target initially");

    install_diagnostic_device(BlockDevice::new(
        "selftest-block0",
        TEST_BLOCK_CAPACITY,
        block_write,
        block_read,
    ));
    let status = diagnostic_status().expect("diagnostic block target installed");
    testrt::check_eq(status.label, "selftest-block0");
    testrt::check_eq(status.capacity_bytes, TEST_BLOCK_CAPACITY);

    let write = write_diagnostic_artifact(b"artifact");
    testrt::check_eq(write.available, true);
    testrt::check_eq(write.storage, "selftest-block0");
    testrt::check_eq(write.capacity_bytes, TEST_BLOCK_CAPACITY);
    testrt::check_eq(write.bytes, 8usize);
    testrt::check_eq(write.ok, true);
    testrt::check_eq(write.reason, "write-ok");
    testrt::check_eq(BLOCK_BUFFER.last_write_offset.load(Ordering::Relaxed), 0usize);

    let mut readback = [0u8; TEST_BLOCK_CAPACITY];
    let read = read_diagnostic_artifact(&mut readback);
    testrt::check_eq(read.available, true);
    testrt::check_eq(read.bytes, 8usize);
    testrt::check_eq(read.ok, true);
    testrt::check_eq(read.reason, "read-ok");
    testrt::check_eq(BLOCK_BUFFER.last_read_offset.load(Ordering::Relaxed), 0usize);
    testrt::check_eq(&readback[..8], b"artifact");

    clear_diagnostic_device_for_tests();
});

arch_test!(diagnostic_block_target_rejects_oversized_artifact, {
    clear_diagnostic_device_for_tests();
    reset_buffer();
    install_diagnostic_device(BlockDevice::new("tiny-block", 4, block_write, block_read));

    let write = write_diagnostic_artifact(b"too-large");
    testrt::check_eq(write.available, true);
    testrt::check_eq(write.storage, "tiny-block");
    testrt::check_eq(write.capacity_bytes, 4usize);
    testrt::check_eq(write.bytes, 0usize);
    testrt::check_eq(write.ok, false);
    testrt::check_eq(write.reason, "write-out-of-range");

    clear_diagnostic_device_for_tests();
});

arch_test!(source_media_target_reads_artifact_separately_from_diagnostic_sink, {
    use core::sync::atomic::Ordering;

    clear_diagnostic_device_for_tests();
    clear_source_media_device_for_tests();
    reset_buffer();
    testrt::check(source_media_status().is_none(), "no source-media target initially");

    install_source_media_device(BlockDevice::new(
        "selftest-source-media0",
        TEST_BLOCK_CAPACITY,
        block_write,
        block_read,
    ));
    let status = source_media_status().expect("source-media target installed");
    testrt::check_eq(status.label, "selftest-source-media0");
    testrt::check_eq(status.capacity_bytes, TEST_BLOCK_CAPACITY);

    let write = block_write(0, b"reovim-source-v1\nexit-status ok\n");
    testrt::check(write, "seed source-media bytes");

    let mut readback = [0u8; TEST_BLOCK_CAPACITY];
    let read = read_source_media_artifact(&mut readback);
    testrt::check_eq(read.available, true);
    testrt::check_eq(read.storage, "selftest-source-media0");
    testrt::check_eq(read.bytes, b"reovim-source-v1\nexit-status ok\n".len());
    testrt::check_eq(read.ok, true);
    testrt::check_eq(read.reason, "read-ok");
    testrt::check_eq(BLOCK_BUFFER.last_read_offset.load(Ordering::Relaxed), 0usize);

    let read = read_source_media_artifact_at(8, &mut readback);
    testrt::check_eq(read.available, true);
    testrt::check_eq(read.storage, "selftest-source-media0");
    testrt::check_eq(read.ok, true);
    testrt::check_eq(read.reason, "read-ok");
    testrt::check_eq(BLOCK_BUFFER.last_read_offset.load(Ordering::Relaxed), 8usize);

    let diagnostic_read = read_diagnostic_artifact(&mut readback);
    testrt::check_eq(diagnostic_read.available, false);
    testrt::check_eq(diagnostic_read.reason, "no-diagnostic-block-device");

    clear_source_media_device_for_tests();
});
