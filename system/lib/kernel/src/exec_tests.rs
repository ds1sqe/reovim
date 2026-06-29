//! Selftests for the kernel exec admission service.

use {
    super::{
        EMPTY_EXEC_LOAD_RECORD, EMPTY_PENDING_EXEC_RECORD, ExecLoadError, ExecLoadKind,
        ExecLoadReason, ExecLoadStatus, load_bin_program, load_payload_by_name, reset,
        snapshot_loads, snapshot_pending, spawn_bin_program, spawn_bin_program_with_stdin,
        spawn_payload_child, take_pending_program,
    },
    crate::{
        block::{
            BlockDevice, clear_exec_bundle_device_for_tests, clear_source_media_device_for_tests,
            install_exec_bundle_device, install_source_media_device,
        },
        exec_artifact::{
            ExecArtifactBodyFormat, ExecArtifactBodyInnerFormat, ExecArtifactFormat,
            ExecArtifactOrigin, install_exec_bundle_bin_descriptors,
            install_exec_bundle_payload_descriptors,
        },
        exec_body::{ExecBodyInnerFormat, parse_exec_body},
        mm, proc,
        program::{
            self, ProgramArgvBuffer, ProgramDescriptor, ProgramEnvBuffer, ProgramImage,
            ProgramImageKind, ProgramSourceArtifact,
        },
        rootd::{self, PayloadDescriptor, PayloadImage, PayloadImageKind, PayloadSourceArtifact},
        source_store::{
            ExecutableSourceStore, MAX_SOURCE_MEDIA_ARTIFACT_BYTES, SourceArtifactNamespace,
            install_source, reset_installed_sources, source_media_checksum32,
        },
    },
    core::sync::atomic::{AtomicUsize, Ordering},
    reovim_testrt::{self as testrt, arch_test},
};

const TEST_PAYLOAD_SOURCE: &[u8] = b"reovim-payload-source-v1\nexit-status ready\n";
const TEST_PAYLOADS: [PayloadDescriptor; 1] = [PayloadDescriptor {
    name: "editor-smoke",
    path: "/payload/editor-smoke",
    summary: "editor smoke payload",
    entry_name: "payload_editor_smoke",
    image: PayloadImage::SourcePath("/payload/editor-smoke"),
}];
const TEST_PAYLOAD_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/editor-smoke",
    kind: PayloadImageKind::SourceImage,
    bytes: TEST_PAYLOAD_SOURCE,
}];

const INVALID_BIN_SOURCE: &[u8] = b"reovim-source-v1\nnot-a-valid-op\n";
const INVALID_BIN_PROGRAMS: [ProgramDescriptor; 1] = [ProgramDescriptor {
    id: 900,
    name: "badbin",
    path: "/bin/badbin",
    summary: "invalid test program",
    image: ProgramImage::SourcePath("/bin/badbin"),
    entry_name: "bin_badbin",
}];
const INVALID_BIN_SOURCES: [ProgramSourceArtifact; 1] = [ProgramSourceArtifact {
    path: "/bin/badbin",
    kind: ProgramImageKind::SourceImage,
    bytes: INVALID_BIN_SOURCE,
}];

const INVALID_PAYLOAD_SOURCE: &[u8] = b"reovim-payload-source-v1\nnot-a-valid-op\n";
const INVALID_PAYLOAD_HEX_SOURCE: &[u8] = b"reovim-payload-source-v1\nwrite-stdout-hex 0\n";
const INVALID_PAYLOAD_STDERR_HEX_SOURCE: &[u8] = b"reovim-payload-source-v1\nwrite-stderr-hex 0\n";
const INVALID_PAYLOAD_STDIN_HEX_SOURCE: &[u8] =
    b"reovim-payload-source-v1\nexec-bin-stdin-hex 0 cat\n";
const INVALID_PAYLOAD_SERVICE_READY_SOURCE: &[u8] = b"reovim-payload-source-v1\nservice-ready \n";
const INVALID_PAYLOAD_SERVICE_HOLD_SOURCE: &[u8] = b"reovim-payload-source-v1\nservice-hold \n";
const INVALID_PAYLOAD_VFS_SOURCE: &[u8] = b"reovim-payload-source-v1\nwrite-vfs-file \n";
const INVALID_PAYLOAD_SPAWN_SOURCE: &[u8] = b"reovim-payload-source-v1\nspawn-bin \n";
const INVALID_PAYLOAD_SPAWN_KILL_SOURCE: &[u8] = b"reovim-payload-source-v1\nspawn-kill-bin \n";
const INVALID_PAYLOAD_SPAWN_WAIT_PAYLOAD_SOURCE: &[u8] =
    b"reovim-payload-source-v1\nspawn-wait-payload \n";
const INVALID_PAYLOAD_SPAWN_KILL_PAYLOAD_SOURCE: &[u8] =
    b"reovim-payload-source-v1\nspawn-kill-payload \n";
const INVALID_PAYLOAD_SLEEP_WAIT_SOURCE: &[u8] =
    b"reovim-payload-source-v1\nsleep-wait-bin 0 pwd\n";
const INVALID_PAYLOAD_SLEEP_SOURCE: &[u8] = b"reovim-payload-source-v1\nsleep-bin 0 pwd\n";
const INVALID_PAYLOADS: [PayloadDescriptor; 1] = [PayloadDescriptor {
    name: "badpayload",
    path: "/payload/badpayload",
    summary: "invalid test payload",
    entry_name: "payload_badpayload",
    image: PayloadImage::SourcePath("/payload/badpayload"),
}];
const INVALID_PAYLOAD_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_SOURCE,
}];
const INVALID_PAYLOAD_HEX_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_HEX_SOURCE,
}];
const INVALID_PAYLOAD_STDERR_HEX_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_STDERR_HEX_SOURCE,
}];
const INVALID_PAYLOAD_STDIN_HEX_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_STDIN_HEX_SOURCE,
}];
const INVALID_PAYLOAD_SERVICE_READY_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_SERVICE_READY_SOURCE,
}];
const INVALID_PAYLOAD_SERVICE_HOLD_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_SERVICE_HOLD_SOURCE,
}];
const INVALID_PAYLOAD_VFS_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_VFS_SOURCE,
}];
const INVALID_PAYLOAD_SPAWN_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_SPAWN_SOURCE,
}];
const INVALID_PAYLOAD_SPAWN_KILL_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_SPAWN_KILL_SOURCE,
}];
const INVALID_PAYLOAD_SPAWN_WAIT_PAYLOAD_SOURCES: [PayloadSourceArtifact; 1] =
    [PayloadSourceArtifact {
        path: "/payload/badpayload",
        kind: PayloadImageKind::SourceImage,
        bytes: INVALID_PAYLOAD_SPAWN_WAIT_PAYLOAD_SOURCE,
    }];
const INVALID_PAYLOAD_SPAWN_KILL_PAYLOAD_SOURCES: [PayloadSourceArtifact; 1] =
    [PayloadSourceArtifact {
        path: "/payload/badpayload",
        kind: PayloadImageKind::SourceImage,
        bytes: INVALID_PAYLOAD_SPAWN_KILL_PAYLOAD_SOURCE,
    }];
const INVALID_PAYLOAD_SLEEP_WAIT_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_SLEEP_WAIT_SOURCE,
}];
const INVALID_PAYLOAD_SLEEP_SOURCES: [PayloadSourceArtifact; 1] = [PayloadSourceArtifact {
    path: "/payload/badpayload",
    kind: PayloadImageKind::SourceImage,
    bytes: INVALID_PAYLOAD_SLEEP_SOURCE,
}];

const MISSING_SOURCE_PROGRAMS: [ProgramDescriptor; 1] = [ProgramDescriptor {
    id: 901,
    name: "nosource",
    path: "/bin/nosource",
    summary: "missing source test program",
    image: ProgramImage::SourcePath("/bin/nosource"),
    entry_name: "bin_nosource",
}];

const MISSING_SOURCE_PAYLOADS: [PayloadDescriptor; 1] = [PayloadDescriptor {
    name: "missing-source",
    path: "/payload/missing-source",
    summary: "missing source test payload",
    entry_name: "payload_missing_source",
    image: PayloadImage::SourcePath("/payload/missing-source"),
}];

const SOURCE_MEDIA_NONE: usize = 0;
const SOURCE_MEDIA_BIN_NOSOURCE: usize = 1;
const SOURCE_MEDIA_PAYLOAD_MISSING: usize = 2;
const SOURCE_MEDIA_BIN_WRONG_PATH: usize = 3;
const SOURCE_MEDIA_BIN_CATALOG: usize = 4;
const SOURCE_MEDIA_BIN_DYNAMIC: usize = 5;
const SOURCE_MEDIA_PAYLOAD_DYNAMIC: usize = 6;
const SOURCE_MEDIA_CATALOG_ARTIFACT_OFFSET: usize = 256;
static SOURCE_MEDIA_KIND: AtomicUsize = AtomicUsize::new(SOURCE_MEDIA_NONE);
const EXEC_BUNDLE_NONE: usize = 0;
const EXEC_BUNDLE_BIN_NOSOURCE: usize = 1;
const EXEC_BUNDLE_PAYLOAD_MISSING: usize = 2;
const EXEC_BUNDLE_BIN_CATALOG: usize = 3;
const EXEC_BUNDLE_PAYLOAD_DYNAMIC: usize = 4;
const EXEC_BUNDLE_BIN_EXEC_BODY: usize = 5;
const EXEC_BUNDLE_PAYLOAD_EXEC_BODY: usize = 6;
const EXEC_BUNDLE_BIN_UAPI_EXEC_BODY: usize = 7;
const EXEC_BUNDLE_CATALOG_ARTIFACT_OFFSET: usize = 256;
static EXEC_BUNDLE_KIND: AtomicUsize = AtomicUsize::new(EXEC_BUNDLE_NONE);

const MEDIA_BIN_SOURCE: &[u8] = b"reovim-source-v1\nexit-status ok\n";
const MEDIA_BIN_UAPI_BODY: &[u8] =
    b"reovim-bin-uapi-v1\nwrite-stdout-hex 62696e2d756170692e6f6b0a\nopen-readonly-write-stdout-env-or-arg1-or REOVIM_MEDIA_PATH /boot/profile\nspawn-wait-bin-env REOVIM_MEDIA_PATH /boot/status hello\nexit-status ok\n";
const MEDIA_PAYLOAD_SOURCE: &[u8] = b"reovim-payload-source-v1\nexec-bin pwd\nexit-status ready\n";

fn source_media_write(_offset: usize, _bytes: &[u8]) -> bool {
    false
}

fn source_media_copy(out: &mut [u8], len: &mut usize, bytes: &[u8]) -> bool {
    if *len + bytes.len() > out.len() {
        return false;
    }
    out[*len..*len + bytes.len()].copy_from_slice(bytes);
    *len += bytes.len();
    true
}

fn source_media_copy_usize(out: &mut [u8], len: &mut usize, mut value: usize) -> bool {
    let mut digits = [0u8; 20];
    let mut digit_count = 0usize;
    if value == 0 {
        digits[0] = b'0';
        digit_count = 1;
    } else {
        while value > 0 {
            digits[digit_count] = b'0' + (value % 10) as u8;
            value /= 10;
            digit_count += 1;
        }
    }
    while digit_count > 0 {
        digit_count -= 1;
        if !source_media_copy(out, len, &digits[digit_count..digit_count + 1]) {
            return false;
        }
    }
    true
}

fn source_media_encode(out: &mut [u8], namespace: &[u8], path: &[u8], source: &[u8]) -> usize {
    let mut len = 0usize;
    let checksum = source_media_checksum32(source) as usize;
    if !source_media_copy(out, &mut len, b"reovim-source-media-v1\nnamespace=")
        || !source_media_copy(out, &mut len, namespace)
        || !source_media_copy(out, &mut len, b"\npath=")
        || !source_media_copy(out, &mut len, path)
        || !source_media_copy(out, &mut len, b"\nbytes=")
        || !source_media_copy_usize(out, &mut len, source.len())
        || !source_media_copy(out, &mut len, b"\nchecksum=")
        || !source_media_copy_usize(out, &mut len, checksum)
        || !source_media_copy(out, &mut len, b"\n")
        || !source_media_copy(out, &mut len, source)
    {
        return 0;
    }
    len
}

fn source_media_catalog_encode(
    out: &mut [u8],
    namespace: &[u8],
    path: &[u8],
    source: &[u8],
    artifact_offset: usize,
) -> usize {
    let mut artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let artifact_len = source_media_encode(&mut artifact, namespace, path, source);
    let artifact_checksum = source_media_checksum32(&artifact[..artifact_len]) as usize;

    let mut body = [0u8; 192];
    let mut body_len = 0usize;
    if !source_media_copy(&mut body, &mut body_len, b"entry namespace=")
        || !source_media_copy(&mut body, &mut body_len, namespace)
        || !source_media_copy(&mut body, &mut body_len, b" path=")
        || !source_media_copy(&mut body, &mut body_len, path)
        || !source_media_copy(&mut body, &mut body_len, b" offset=")
        || !source_media_copy_usize(&mut body, &mut body_len, artifact_offset)
        || !source_media_copy(&mut body, &mut body_len, b" bytes=")
        || !source_media_copy_usize(&mut body, &mut body_len, artifact_len)
        || !source_media_copy(&mut body, &mut body_len, b" checksum=")
        || !source_media_copy_usize(&mut body, &mut body_len, artifact_checksum)
        || !source_media_copy(&mut body, &mut body_len, b"\n")
    {
        return 0;
    }

    let checksum = source_media_checksum32(&body[..body_len]) as usize;
    let mut len = 0usize;
    if !source_media_copy(out, &mut len, b"reovim-source-media-catalog-v1\nbytes=")
        || !source_media_copy_usize(out, &mut len, body_len)
        || !source_media_copy(out, &mut len, b"\nchecksum=")
        || !source_media_copy_usize(out, &mut len, checksum)
        || !source_media_copy(out, &mut len, b"\n")
        || !source_media_copy(out, &mut len, &body[..body_len])
    {
        return 0;
    }
    len
}

fn source_media_read(offset: usize, out: &mut [u8]) -> usize {
    let kind = SOURCE_MEDIA_KIND.load(Ordering::Relaxed);
    if kind == SOURCE_MEDIA_BIN_CATALOG {
        if offset == 0 {
            return source_media_catalog_encode(
                out,
                b"bin",
                b"/bin/nosource",
                MEDIA_BIN_SOURCE,
                SOURCE_MEDIA_CATALOG_ARTIFACT_OFFSET,
            );
        }
        if offset == SOURCE_MEDIA_CATALOG_ARTIFACT_OFFSET {
            return source_media_encode(out, b"bin", b"/bin/nosource", MEDIA_BIN_SOURCE);
        }
        return 0;
    }

    if offset != 0 {
        return 0;
    }
    let (namespace, path, source) = match kind {
        SOURCE_MEDIA_BIN_NOSOURCE => {
            (b"bin".as_slice(), b"/bin/nosource".as_slice(), MEDIA_BIN_SOURCE)
        }
        SOURCE_MEDIA_PAYLOAD_MISSING => (
            b"payload".as_slice(),
            b"/payload/missing-source".as_slice(),
            MEDIA_PAYLOAD_SOURCE,
        ),
        SOURCE_MEDIA_BIN_WRONG_PATH => {
            (b"bin".as_slice(), b"/bin/other".as_slice(), MEDIA_BIN_SOURCE)
        }
        SOURCE_MEDIA_BIN_DYNAMIC => {
            (b"bin".as_slice(), b"/bin/media-bin".as_slice(), MEDIA_BIN_SOURCE)
        }
        SOURCE_MEDIA_PAYLOAD_DYNAMIC => (
            b"payload".as_slice(),
            b"/payload/media-payload".as_slice(),
            MEDIA_PAYLOAD_SOURCE,
        ),
        _ => return 0,
    };
    source_media_encode(out, namespace, path, source)
}

fn install_source_media(kind: usize) {
    SOURCE_MEDIA_KIND.store(kind, Ordering::Relaxed);
    install_source_media_device(BlockDevice::new(
        "exec-test-source-media0",
        MAX_SOURCE_MEDIA_ARTIFACT_BYTES,
        source_media_write,
        source_media_read,
    ));
}

fn clear_source_media() {
    SOURCE_MEDIA_KIND.store(SOURCE_MEDIA_NONE, Ordering::Relaxed);
    clear_source_media_device_for_tests();
}

fn exec_bundle_encode(out: &mut [u8], namespace: &[u8], path: &[u8], source: &[u8]) -> usize {
    let mut len = 0usize;
    let checksum = source_media_checksum32(source) as usize;
    if !source_media_copy(out, &mut len, b"reovim-exec-bundle-v1\nnamespace=")
        || !source_media_copy(out, &mut len, namespace)
        || !source_media_copy(out, &mut len, b"\npath=")
        || !source_media_copy(out, &mut len, path)
        || !source_media_copy(out, &mut len, b"\nbytes=")
        || !source_media_copy_usize(out, &mut len, source.len())
        || !source_media_copy(out, &mut len, b"\nchecksum=")
        || !source_media_copy_usize(out, &mut len, checksum)
        || !source_media_copy(out, &mut len, b"\n")
        || !source_media_copy(out, &mut len, source)
    {
        return 0;
    }
    len
}

fn exec_body_encode(out: &mut [u8], inner: &[u8], body: &[u8]) -> usize {
    let mut len = 0usize;
    let checksum = source_media_checksum32(body) as usize;
    if !source_media_copy(out, &mut len, b"reovim-exec-body-v1\ninner=")
        || !source_media_copy(out, &mut len, inner)
        || !source_media_copy(out, &mut len, b"\nbytes=")
        || !source_media_copy_usize(out, &mut len, body.len())
        || !source_media_copy(out, &mut len, b"\nchecksum=")
        || !source_media_copy_usize(out, &mut len, checksum)
        || !source_media_copy(out, &mut len, b"\n")
        || !source_media_copy(out, &mut len, body)
    {
        return 0;
    }
    len
}

fn exec_bundle_encode_exec_body(
    out: &mut [u8],
    namespace: &[u8],
    path: &[u8],
    inner: &[u8],
    body: &[u8],
) -> usize {
    let mut exec_body = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let exec_body_len = exec_body_encode(&mut exec_body, inner, body);
    exec_bundle_encode(out, namespace, path, &exec_body[..exec_body_len])
}

fn exec_bundle_catalog_encode(
    out: &mut [u8],
    namespace: &[u8],
    path: &[u8],
    source: &[u8],
    artifact_offset: usize,
) -> usize {
    let mut artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let artifact_len = exec_bundle_encode(&mut artifact, namespace, path, source);
    let artifact_checksum = source_media_checksum32(&artifact[..artifact_len]) as usize;

    let mut body = [0u8; 192];
    let mut body_len = 0usize;
    if !source_media_copy(&mut body, &mut body_len, b"entry namespace=")
        || !source_media_copy(&mut body, &mut body_len, namespace)
        || !source_media_copy(&mut body, &mut body_len, b" path=")
        || !source_media_copy(&mut body, &mut body_len, path)
        || !source_media_copy(&mut body, &mut body_len, b" offset=")
        || !source_media_copy_usize(&mut body, &mut body_len, artifact_offset)
        || !source_media_copy(&mut body, &mut body_len, b" bytes=")
        || !source_media_copy_usize(&mut body, &mut body_len, artifact_len)
        || !source_media_copy(&mut body, &mut body_len, b" checksum=")
        || !source_media_copy_usize(&mut body, &mut body_len, artifact_checksum)
        || !source_media_copy(&mut body, &mut body_len, b"\n")
    {
        return 0;
    }

    let checksum = source_media_checksum32(&body[..body_len]) as usize;
    let mut len = 0usize;
    if !source_media_copy(out, &mut len, b"reovim-exec-bundle-catalog-v1\nbytes=")
        || !source_media_copy_usize(out, &mut len, body_len)
        || !source_media_copy(out, &mut len, b"\nchecksum=")
        || !source_media_copy_usize(out, &mut len, checksum)
        || !source_media_copy(out, &mut len, b"\n")
        || !source_media_copy(out, &mut len, &body[..body_len])
    {
        return 0;
    }
    len
}

fn exec_bundle_write(_offset: usize, _bytes: &[u8]) -> bool {
    false
}

fn exec_bundle_read(offset: usize, out: &mut [u8]) -> usize {
    match EXEC_BUNDLE_KIND.load(Ordering::Relaxed) {
        EXEC_BUNDLE_BIN_NOSOURCE if offset == 0 => {
            exec_bundle_encode(out, b"bin", b"/bin/nosource", MEDIA_BIN_SOURCE)
        }
        EXEC_BUNDLE_PAYLOAD_MISSING if offset == 0 => {
            exec_bundle_encode(out, b"payload", b"/payload/missing-source", MEDIA_PAYLOAD_SOURCE)
        }
        EXEC_BUNDLE_PAYLOAD_DYNAMIC if offset == 0 => {
            exec_bundle_encode(out, b"payload", b"/payload/media-payload", MEDIA_PAYLOAD_SOURCE)
        }
        EXEC_BUNDLE_BIN_EXEC_BODY if offset == 0 => exec_bundle_encode_exec_body(
            out,
            b"bin",
            b"/bin/nosource",
            b"bin-source-image",
            MEDIA_BIN_SOURCE,
        ),
        EXEC_BUNDLE_BIN_UAPI_EXEC_BODY if offset == 0 => exec_bundle_encode_exec_body(
            out,
            b"bin",
            b"/bin/nosource",
            b"bin-uapi-v1",
            MEDIA_BIN_UAPI_BODY,
        ),
        EXEC_BUNDLE_PAYLOAD_EXEC_BODY if offset == 0 => exec_bundle_encode_exec_body(
            out,
            b"payload",
            b"/payload/missing-source",
            b"payload-source-image",
            MEDIA_PAYLOAD_SOURCE,
        ),
        EXEC_BUNDLE_BIN_CATALOG if offset == 0 => exec_bundle_catalog_encode(
            out,
            b"bin",
            b"/bin/nosource",
            MEDIA_BIN_SOURCE,
            EXEC_BUNDLE_CATALOG_ARTIFACT_OFFSET,
        ),
        EXEC_BUNDLE_BIN_CATALOG if offset == EXEC_BUNDLE_CATALOG_ARTIFACT_OFFSET => {
            exec_bundle_encode(out, b"bin", b"/bin/nosource", MEDIA_BIN_SOURCE)
        }
        _ => 0,
    }
}

fn install_exec_bundle(kind: usize) {
    EXEC_BUNDLE_KIND.store(kind, Ordering::Relaxed);
    install_exec_bundle_device(BlockDevice::new(
        "exec-test-bundle0",
        MAX_SOURCE_MEDIA_ARTIFACT_BYTES,
        exec_bundle_write,
        exec_bundle_read,
    ));
}

fn clear_exec_bundle() {
    EXEC_BUNDLE_KIND.store(EXEC_BUNDLE_NONE, Ordering::Relaxed);
    clear_exec_bundle_device_for_tests();
}

fn argv1(arg0: &str) -> ProgramArgvBuffer {
    let mut argv = ProgramArgvBuffer::empty();
    argv.push(arg0).expect("test argv0 fits");
    argv
}

fn process_handle(pid: usize) -> proc::ProcessHandle {
    let record = proc::process(pid).expect("process record exists");
    proc::ProcessHandle {
        pid: record.pid,
        task_id: record.task_id,
        program_path: record.program_path,
        loader: record.loader,
        entry_name: record.entry_name,
    }
}

fn bin_store() -> ExecutableSourceStore {
    ExecutableSourceStore::program_only(crate::bin_fixture::program_sources())
}

fn program_store(sources: &'static [ProgramSourceArtifact]) -> ExecutableSourceStore {
    ExecutableSourceStore::program_only(sources)
}

fn payload_store(sources: &'static [PayloadSourceArtifact]) -> ExecutableSourceStore {
    ExecutableSourceStore::payload_only(sources)
}

arch_test!(exec_loader_resolves_bin_argv0, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_source_media();

    let help = load_bin_program(crate::bin_fixture::programs(), bin_store(), "help")
        .expect("help loads by basename");
    testrt::check_eq(help.descriptor.path, "/bin/help");
    testrt::check_eq(help.descriptor.entry_name, "bin_help");
    testrt::check_eq(help.image_kind, help.descriptor.image_kind());
    testrt::check_eq(help.source_path, "/bin/help");

    let by_path = load_bin_program(crate::bin_fixture::programs(), bin_store(), "/bin/help")
        .expect("help loads by absolute path");
    testrt::check_eq(by_path.catalog_index, help.catalog_index);
    testrt::check_eq(
        load_bin_program(crate::bin_fixture::programs(), bin_store(), "").err(),
        Some(ExecLoadError::EmptyArgv0),
    );
    testrt::check_eq(
        load_bin_program(crate::bin_fixture::programs(), bin_store(), "missing").err(),
        Some(ExecLoadError::NotFound),
    );
    testrt::check_eq(
        load_bin_program(crate::bin_fixture::programs(), bin_store(), "/boot/help").err(),
        Some(ExecLoadError::NotFound),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 5usize);
    testrt::check_eq(records[0].argv0(), "help");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/bin/help");
    testrt::check_eq(records[0].loader, help.descriptor.image_kind().as_str());
    testrt::check_eq(records[0].source_path, "/bin/help");
    testrt::check_eq(records[0].entry_name, "bin_help");
    testrt::check_eq(records[2].argv0(), "");
    testrt::check_eq(records[2].status, ExecLoadStatus::Error);
    testrt::check_eq(records[2].reason, ExecLoadReason::EmptyArgv0);
    testrt::check_eq(records[2].kind, ExecLoadKind::None);
    testrt::check_eq(records[2].origin, ExecArtifactOrigin::None);
    testrt::check_eq(records[3].argv0(), "missing");
    testrt::check_eq(records[3].status, ExecLoadStatus::Error);
    testrt::check_eq(records[3].reason, ExecLoadReason::NotFound);
    testrt::check_eq(records[3].kind, ExecLoadKind::None);
    testrt::check_eq(records[3].origin, ExecArtifactOrigin::None);
    testrt::check_eq(records[4].argv0(), "/boot/help");
    testrt::check_eq(records[4].status, ExecLoadStatus::Error);
    testrt::check_eq(records[4].reason, ExecLoadReason::NotFound);
    testrt::check_eq(records[4].kind, ExecLoadKind::None);
    testrt::check_eq(records[4].origin, ExecArtifactOrigin::None);
});

arch_test!(exec_loader_records_source_image_programs, {
    proc::reset();
    reset();

    let ls = load_bin_program(crate::bin_fixture::programs(), bin_store(), "ls")
        .expect("ls loads by basename");
    testrt::check_eq(ls.descriptor.path, "/bin/ls");
    testrt::check_eq(ls.image_kind.as_str(), "linked-bin");
    testrt::check_eq(ls.source_path, "/bin/ls");
    let clear = load_bin_program(crate::bin_fixture::programs(), bin_store(), "clear")
        .expect("clear loads by basename");
    testrt::check_eq(clear.descriptor.path, "/bin/clear");
    testrt::check_eq(clear.image_kind.as_str(), "linked-bin");
    testrt::check_eq(clear.source_path, "/bin/clear");

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(records[0].argv0(), "ls");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/bin/ls");
    testrt::check_eq(records[0].loader, "linked-bin");
    testrt::check_eq(records[0].source_path, "/bin/ls");
    testrt::check_eq(records[0].entry_name, "bin_ls");
    testrt::check_eq(records[0].artifact_format, ExecArtifactFormat::Direct);
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::LinkedImage);
    testrt::check_eq(records[0].artifact_body_inner_format, ExecArtifactBodyInnerFormat::None);
    testrt::check_eq(records[0].artifact_bytes_len, 0usize);
    testrt::check_eq(records[0].artifact_body_bytes_len, 0usize);
    testrt::check_eq(records[0].artifact_checksum, 0u32);
    testrt::check_eq(records[1].argv0(), "clear");
    testrt::check_eq(records[1].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[1].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[1].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[1].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[1].path, "/bin/clear");
    testrt::check_eq(records[1].loader, "linked-bin");
    testrt::check_eq(records[1].source_path, "/bin/clear");
    testrt::check_eq(records[1].entry_name, "bin_clear");
    testrt::check_eq(records[1].artifact_format, ExecArtifactFormat::Direct);
    testrt::check_eq(records[1].artifact_body_format, ExecArtifactBodyFormat::LinkedImage);
    testrt::check_eq(records[1].artifact_body_inner_format, ExecArtifactBodyInnerFormat::None);
    testrt::check_eq(records[1].artifact_bytes_len, 0usize);
    testrt::check_eq(records[1].artifact_body_bytes_len, 0usize);
    testrt::check_eq(records[1].artifact_checksum, 0u32);

    let handle = spawn_bin_program(ls, argv1("ls"));
    let process = proc::process(handle.pid).expect("linked process retained");
    testrt::check_eq(process.loader, "linked-bin");
    testrt::check_eq(process.artifact_body_format, ExecArtifactBodyFormat::LinkedImage);
    testrt::check_eq(process.artifact_body_inner_format, ExecArtifactBodyInnerFormat::None);
});

arch_test!(exec_loader_rejects_invalid_bin_source_image_at_admission, {
    proc::reset();
    reset();

    testrt::check_eq(
        load_bin_program(&INVALID_BIN_PROGRAMS, program_store(&INVALID_BIN_SOURCES), "badbin")
            .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badbin");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/bin/badbin");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/bin/badbin");
    testrt::check_eq(records[0].entry_name, "bin_badbin");
});

arch_test!(exec_loader_rejects_missing_bin_source_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_source_media();

    testrt::check_eq(
        load_bin_program(&MISSING_SOURCE_PROGRAMS, program_store(&[]), "nosource").err(),
        Some(ExecLoadError::SourceNotFound),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "nosource");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::SourceNotFound);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::None);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/bin/nosource");
    testrt::check_eq(records[0].entry_name, "bin_nosource");
    reset_installed_sources();
    clear_source_media();
});

arch_test!(exec_loader_loads_missing_bin_source_from_source_media, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_BIN_NOSOURCE);

    let loaded = load_bin_program(&MISSING_SOURCE_PROGRAMS, program_store(&[]), "nosource")
        .expect("missing source loads from matching source media");
    testrt::check_eq(loaded.descriptor.path, "/bin/nosource");
    testrt::check_eq(loaded.source_path, "/bin/nosource");
    testrt::check_eq(loaded.source_bytes(), MEDIA_BIN_SOURCE);

    let handle = spawn_bin_program(loaded, argv1("nosource"));
    let process = proc::process(handle.pid).expect("spawned process retained");
    let space = mm::address_space(process.address_space_id).expect("spawned address space mapped");
    testrt::check_eq(space.source_path, "/bin/nosource");
    testrt::check_eq(space.text_bytes, MEDIA_BIN_SOURCE.len());
    testrt::check_eq(space.text_checksum, source_media_checksum32(MEDIA_BIN_SOURCE));
    testrt::check_eq(space.stack_bytes, mm::DEFAULT_USER_STACK_BYTES);
    let expected_text_len = ((MEDIA_BIN_SOURCE.len() + mm::USER_PAGE_BYTES - 1)
        / mm::USER_PAGE_BYTES)
        * mm::USER_PAGE_BYTES;
    testrt::check_eq(space.region_count, 2usize);
    testrt::check_eq(space.text_start, mm::USER_TEXT_BASE);
    testrt::check_eq(space.text_end, mm::USER_TEXT_BASE + expected_text_len);
    testrt::check_eq(space.text_flags, mm::ADDRESS_SPACE_TEXT_FLAGS);
    testrt::check_eq(space.stack_start, mm::USER_STACK_TOP - mm::DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(space.stack_end, mm::USER_STACK_TOP);
    testrt::check_eq(space.stack_flags, mm::ADDRESS_SPACE_STACK_FLAGS);
    testrt::check_eq(space.page_table_id, process.address_space_id);
    testrt::check_eq(space.mapped_pages, 5usize);
    testrt::check_eq(space.text_pages, 1usize);
    testrt::check_eq(space.stack_pages, 4usize);
    let mut page_tables =
        [mm::EMPTY_ADDRESS_SPACE_PAGE_TABLE_RECORD; mm::MAX_ADDRESS_SPACE_PAGE_TABLE_RECORDS];
    let page_table_count = mm::snapshot_address_space_page_tables(&mut page_tables);
    testrt::check_eq(page_table_count, 1usize);
    testrt::check_eq(page_tables[0].id, process.address_space_id);
    testrt::check_eq(page_tables[0].address_space_id, process.address_space_id);
    testrt::check_eq(page_tables[0].owner_pid, handle.pid);
    testrt::check_eq(page_tables[0].state, mm::AddressSpaceState::Active);
    testrt::check_eq(page_tables[0].root_table_id, process.address_space_id);
    testrt::check_eq(page_tables[0].mapped_pages, 5usize);
    testrt::check_eq(page_tables[0].text_pages, 1usize);
    testrt::check_eq(page_tables[0].stack_pages, 4usize);
    testrt::check_eq(page_tables[0].text_flags, mm::ADDRESS_SPACE_TEXT_FLAGS);
    testrt::check_eq(page_tables[0].stack_flags, mm::ADDRESS_SPACE_STACK_FLAGS);
    let mut objects = [mm::EMPTY_ADDRESS_SPACE_OBJECT_RECORD; mm::MAX_ADDRESS_SPACE_OBJECT_RECORDS];
    let object_count = mm::snapshot_address_space_objects(&mut objects);
    testrt::check_eq(object_count, 2usize);
    testrt::check_eq(objects[0].address_space_id, process.address_space_id);
    testrt::check_eq(objects[0].owner_pid, handle.pid);
    testrt::check_eq(objects[0].state, mm::AddressSpaceState::Active);
    testrt::check_eq(objects[0].kind, mm::AddressSpaceObjectKind::Text);
    testrt::check_eq(objects[0].backing, mm::AddressSpaceObjectBacking::SourceText);
    testrt::check_eq(objects[0].source_path, "/bin/nosource");
    testrt::check_eq(objects[0].start, mm::USER_TEXT_BASE);
    testrt::check_eq(objects[0].end, mm::USER_TEXT_BASE + expected_text_len);
    testrt::check_eq(objects[0].bytes, MEDIA_BIN_SOURCE.len());
    testrt::check_eq(objects[0].checksum, source_media_checksum32(MEDIA_BIN_SOURCE));
    testrt::check_eq(objects[0].flags, mm::ADDRESS_SPACE_TEXT_FLAGS);
    testrt::check_eq(objects[0].page_count, 1usize);
    testrt::check_eq(objects[1].address_space_id, process.address_space_id);
    testrt::check_eq(objects[1].kind, mm::AddressSpaceObjectKind::Stack);
    testrt::check_eq(objects[1].backing, mm::AddressSpaceObjectBacking::ZeroFill);
    testrt::check_eq(objects[1].bytes, mm::DEFAULT_USER_STACK_BYTES);
    testrt::check_eq(objects[1].flags, mm::ADDRESS_SPACE_STACK_FLAGS);
    testrt::check_eq(objects[1].page_count, 4usize);

    let installed = program_store(&[])
        .find_program("/bin/nosource")
        .expect("source media installed source overlay");
    testrt::check_eq(installed.bytes, MEDIA_BIN_SOURCE);

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "nosource");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::LoadedFromSourceMedia);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::SourceMediaSingle);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].source_path, "/bin/nosource");
    let mut expected_artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_artifact_len =
        source_media_encode(&mut expected_artifact, b"bin", b"/bin/nosource", MEDIA_BIN_SOURCE);
    testrt::check_eq(records[0].artifact_format, ExecArtifactFormat::SingleArtifact);
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);
    testrt::check_eq(records[0].artifact_body_inner_format, ExecArtifactBodyInnerFormat::None);
    testrt::check_eq(records[0].artifact_bytes_len, expected_artifact_len);
    testrt::check_eq(records[0].artifact_body_bytes_len, MEDIA_BIN_SOURCE.len());
    testrt::check_eq(records[0].artifact_checksum, source_media_checksum32(MEDIA_BIN_SOURCE));

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
});

arch_test!(exec_loader_loads_missing_bin_source_from_exec_bundle, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_BIN_NOSOURCE);

    let loaded = load_bin_program(&MISSING_SOURCE_PROGRAMS, program_store(&[]), "nosource")
        .expect("missing source loads from matching executable bundle");
    testrt::check_eq(loaded.descriptor.path, "/bin/nosource");
    testrt::check_eq(loaded.source_path, "/bin/nosource");
    testrt::check_eq(loaded.source_bytes(), MEDIA_BIN_SOURCE);

    testrt::check(
        program_store(&[]).find_program("/bin/nosource").is_none(),
        "exec bundle body cache does not install source overlay",
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "nosource");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].source_path, "/bin/nosource");
    let mut expected_artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_artifact_len =
        exec_bundle_encode(&mut expected_artifact, b"bin", b"/bin/nosource", MEDIA_BIN_SOURCE);
    testrt::check_eq(records[0].artifact_format, ExecArtifactFormat::SingleArtifact);
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);
    testrt::check_eq(records[0].artifact_bytes_len, expected_artifact_len);
    testrt::check_eq(records[0].artifact_body_bytes_len, MEDIA_BIN_SOURCE.len());
    testrt::check_eq(records[0].artifact_checksum, source_media_checksum32(MEDIA_BIN_SOURCE));

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
});

arch_test!(exec_loader_loads_missing_bin_exec_body_from_exec_bundle, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_BIN_EXEC_BODY);

    let loaded = load_bin_program(&MISSING_SOURCE_PROGRAMS, program_store(&[]), "nosource")
        .expect("missing source loads from matching executable body bundle");
    testrt::check_eq(loaded.descriptor.path, "/bin/nosource");
    testrt::check_eq(loaded.source_path, "/bin/nosource");
    testrt::check_eq(loaded.image_kind, ProgramImageKind::ReovimExecBody);
    testrt::check_eq(loaded.image_kind.as_str(), "reovim-exec-body");
    let parsed = parse_exec_body(loaded.source_bytes()).expect("exec body bytes parse");
    testrt::check_eq(parsed.inner, ExecBodyInnerFormat::BinSourceImage);
    testrt::check_eq(parsed.bytes, MEDIA_BIN_SOURCE);

    let handle = spawn_bin_program(loaded, argv1("nosource"));
    let process = proc::process(handle.pid).expect("spawned process retained");
    testrt::check_eq(process.loader, "reovim-exec-body");
    testrt::check_eq(process.artifact_body_format, ExecArtifactBodyFormat::ReovimExecBody);
    testrt::check_eq(
        process.artifact_body_inner_format,
        ExecArtifactBodyInnerFormat::BinSourceImage,
    );
    let space = mm::address_space(process.address_space_id).expect("spawned address space mapped");
    testrt::check_eq(space.source_path, "/bin/nosource");
    testrt::check_eq(space.text_bytes, loaded.source_bytes().len());
    testrt::check_eq(space.text_checksum, source_media_checksum32(loaded.source_bytes()));

    testrt::check(
        program_store(&[]).find_program("/bin/nosource").is_none(),
        "exec body bundle does not install source overlay",
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "nosource");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].loader, "reovim-exec-body");
    testrt::check_eq(records[0].source_path, "/bin/nosource");
    let mut expected_body = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_body_len =
        exec_body_encode(&mut expected_body, b"bin-source-image", MEDIA_BIN_SOURCE);
    let mut expected_artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_artifact_len = exec_bundle_encode(
        &mut expected_artifact,
        b"bin",
        b"/bin/nosource",
        &expected_body[..expected_body_len],
    );
    testrt::check_eq(records[0].artifact_format, ExecArtifactFormat::SingleArtifact);
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::ReovimExecBody);
    testrt::check_eq(
        records[0].artifact_body_inner_format,
        ExecArtifactBodyInnerFormat::BinSourceImage,
    );
    testrt::check_eq(records[0].artifact_body_inner_format.as_str(), "bin-source-image");
    testrt::check_eq(records[0].artifact_bytes_len, expected_artifact_len);
    testrt::check_eq(records[0].artifact_body_bytes_len, expected_body_len);
    testrt::check_eq(
        records[0].artifact_checksum,
        source_media_checksum32(&expected_body[..expected_body_len]),
    );

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
});

arch_test!(exec_loader_loads_missing_bin_uapi_exec_body_from_exec_bundle, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_EXEC_BODY);

    let loaded = load_bin_program(&MISSING_SOURCE_PROGRAMS, program_store(&[]), "nosource")
        .expect("missing source loads from bin uapi executable body bundle");
    testrt::check_eq(loaded.descriptor.path, "/bin/nosource");
    testrt::check_eq(loaded.source_path, "/bin/nosource");
    testrt::check_eq(loaded.image_kind, ProgramImageKind::ReovimExecBody);
    let parsed = parse_exec_body(loaded.source_bytes()).expect("bin uapi exec body bytes parse");
    testrt::check_eq(parsed.inner, ExecBodyInnerFormat::BinUapiV1);
    testrt::check_eq(parsed.bytes, MEDIA_BIN_UAPI_BODY);

    let handle = spawn_bin_program(loaded, argv1("nosource"));
    let process = proc::process(handle.pid).expect("spawned process retained");
    testrt::check_eq(process.loader, "reovim-exec-body");
    testrt::check_eq(process.artifact_body_format, ExecArtifactBodyFormat::ReovimExecBody);
    testrt::check_eq(process.artifact_body_inner_format, ExecArtifactBodyInnerFormat::BinUapiV1);
    let space = mm::address_space(process.address_space_id).expect("spawned address space mapped");
    testrt::check_eq(space.source_path, "/bin/nosource");
    testrt::check_eq(space.text_bytes, loaded.source_bytes().len());
    testrt::check_eq(space.text_checksum, source_media_checksum32(loaded.source_bytes()));

    testrt::check(
        program_store(&[]).find_program("/bin/nosource").is_none(),
        "bin uapi exec body bundle does not install source overlay",
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "nosource");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].loader, "reovim-exec-body");
    testrt::check_eq(records[0].source_path, "/bin/nosource");
    let mut expected_body = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_body_len =
        exec_body_encode(&mut expected_body, b"bin-uapi-v1", MEDIA_BIN_UAPI_BODY);
    let mut expected_artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_artifact_len = exec_bundle_encode(
        &mut expected_artifact,
        b"bin",
        b"/bin/nosource",
        &expected_body[..expected_body_len],
    );
    testrt::check_eq(records[0].artifact_format, ExecArtifactFormat::SingleArtifact);
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::ReovimExecBody);
    testrt::check_eq(records[0].artifact_body_inner_format, ExecArtifactBodyInnerFormat::BinUapiV1);
    testrt::check_eq(records[0].artifact_body_inner_format.as_str(), "bin-uapi-v1");
    testrt::check_eq(records[0].artifact_bytes_len, expected_artifact_len);
    testrt::check_eq(records[0].artifact_body_bytes_len, expected_body_len);
    testrt::check_eq(
        records[0].artifact_checksum,
        source_media_checksum32(&expected_body[..expected_body_len]),
    );

    let mut pending = [EMPTY_PENDING_EXEC_RECORD; super::MAX_PENDING_EXEC_RECORDS];
    let pending_count = snapshot_pending(&mut pending);
    testrt::check_eq(pending_count, 1usize);
    testrt::check_eq(pending[0].pid, handle.pid);
    testrt::check_eq(pending[0].path, "/bin/nosource");
    testrt::check_eq(pending[0].loader, "reovim-exec-body");
    testrt::check_eq(pending[0].artifact_body_format, ExecArtifactBodyFormat::ReovimExecBody);
    testrt::check_eq(pending[0].artifact_body_inner_format, ExecArtifactBodyInnerFormat::BinUapiV1);

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
});

arch_test!(exec_loader_loads_missing_bin_source_from_exec_bundle_catalog, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_BIN_CATALOG);

    let loaded = load_bin_program(&MISSING_SOURCE_PROGRAMS, program_store(&[]), "nosource")
        .expect("missing source loads from matching executable bundle catalog");
    testrt::check_eq(loaded.descriptor.path, "/bin/nosource");
    testrt::check_eq(loaded.source_path, "/bin/nosource");
    testrt::check_eq(loaded.source_bytes(), MEDIA_BIN_SOURCE);

    testrt::check(
        program_store(&[]).find_program("/bin/nosource").is_none(),
        "exec bundle catalog body cache does not install source overlay",
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "nosource");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].source_path, "/bin/nosource");
    let mut expected_artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_artifact_len =
        exec_bundle_encode(&mut expected_artifact, b"bin", b"/bin/nosource", MEDIA_BIN_SOURCE);
    testrt::check_eq(records[0].artifact_format, ExecArtifactFormat::Catalog);
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);
    testrt::check_eq(records[0].artifact_bytes_len, expected_artifact_len);
    testrt::check_eq(records[0].artifact_body_bytes_len, MEDIA_BIN_SOURCE.len());
    testrt::check_eq(records[0].artifact_checksum, source_media_checksum32(MEDIA_BIN_SOURCE));

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
});

arch_test!(exec_loader_loads_missing_payload_exec_body_from_exec_bundle, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_PAYLOAD_EXEC_BODY);

    let loaded =
        load_payload_by_name(&MISSING_SOURCE_PAYLOADS, payload_store(&[]), "missing-source")
            .expect("missing payload loads from matching executable body bundle");
    testrt::check_eq(loaded.name, "missing-source");
    testrt::check_eq(loaded.path, "/payload/missing-source");
    testrt::check_eq(loaded.source_path, "/payload/missing-source");
    testrt::check_eq(loaded.image_kind, PayloadImageKind::ReovimExecBody);
    testrt::check_eq(loaded.image_kind.as_str(), "reovim-exec-body");
    let parsed = parse_exec_body(loaded.source_bytes()).expect("payload exec body bytes parse");
    testrt::check_eq(parsed.inner, ExecBodyInnerFormat::PayloadSourceImage);
    testrt::check_eq(parsed.bytes, MEDIA_PAYLOAD_SOURCE);

    testrt::check(
        payload_store(&[])
            .find_payload("/payload/missing-source")
            .is_none(),
        "payload exec body bundle does not install source overlay",
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "missing-source");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/payload/missing-source");
    testrt::check_eq(records[0].loader, "reovim-exec-body");
    testrt::check_eq(records[0].source_path, "/payload/missing-source");
    let mut expected_body = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_body_len =
        exec_body_encode(&mut expected_body, b"payload-source-image", MEDIA_PAYLOAD_SOURCE);
    let mut expected_artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_artifact_len = exec_bundle_encode(
        &mut expected_artifact,
        b"payload",
        b"/payload/missing-source",
        &expected_body[..expected_body_len],
    );
    testrt::check_eq(records[0].artifact_format, ExecArtifactFormat::SingleArtifact);
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::ReovimExecBody);
    testrt::check_eq(
        records[0].artifact_body_inner_format,
        ExecArtifactBodyInnerFormat::PayloadSourceImage,
    );
    testrt::check_eq(records[0].artifact_body_inner_format.as_str(), "payload-source-image");
    testrt::check_eq(records[0].artifact_bytes_len, expected_artifact_len);
    testrt::check_eq(records[0].artifact_body_bytes_len, expected_body_len);
    testrt::check_eq(
        records[0].artifact_checksum,
        source_media_checksum32(&expected_body[..expected_body_len]),
    );

    let parent = process_handle(proc::ROOTD_PID);
    let child = spawn_payload_child(parent, loaded);
    let process = proc::process(child.pid).expect("spawned payload process retained");
    testrt::check_eq(process.artifact_body_format, ExecArtifactBodyFormat::ReovimExecBody);
    testrt::check_eq(
        process.artifact_body_inner_format,
        ExecArtifactBodyInnerFormat::PayloadSourceImage,
    );
    let mut pending = [EMPTY_PENDING_EXEC_RECORD; super::MAX_PENDING_EXEC_RECORDS];
    let pending_count = snapshot_pending(&mut pending);
    testrt::check_eq(pending_count, 1usize);
    testrt::check_eq(pending[0].pid, child.pid);
    testrt::check_eq(pending[0].path, "/payload/missing-source");
    testrt::check_eq(pending[0].loader, "reovim-exec-body");
    testrt::check_eq(pending[0].artifact_body_format, ExecArtifactBodyFormat::ReovimExecBody);
    testrt::check_eq(
        pending[0].artifact_body_inner_format,
        ExecArtifactBodyInnerFormat::PayloadSourceImage,
    );

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
});

arch_test!(exec_bundle_catalog_preinstalls_bin_descriptor_without_source_bytes, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_BIN_CATALOG);

    let install = install_exec_bundle_bin_descriptors();
    testrt::check(install.available, "exec bundle catalog was available");
    testrt::check_eq(install.installed, 1usize);
    testrt::check(!install.truncated, "single bundle descriptor is not truncated");

    let descriptor = program::resolve_argv0(&[], "nosource")
        .expect("preinstalled exec-bundle descriptor resolves before exec")
        .1;
    testrt::check_eq(descriptor.path, "/bin/nosource");
    testrt::check_eq(descriptor.entry_name, "bin_nosource");
    testrt::check(
        program_store(&[]).find_program("/bin/nosource").is_none(),
        "descriptor preinstall does not install source bytes",
    );

    let loaded = load_bin_program(&[], program_store(&[]), "nosource")
        .expect("preinstalled descriptor still loads from exec bundle");
    testrt::check_eq(loaded.source_bytes(), MEDIA_BIN_SOURCE);

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    reset();
});

arch_test!(exec_bundle_catalog_preinstalls_payload_descriptor_without_source_bytes, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_PAYLOAD_DYNAMIC);

    let install = install_exec_bundle_payload_descriptors();
    testrt::check(install.available, "exec bundle catalog was available");
    testrt::check_eq(install.installed, 1usize);
    testrt::check(!install.truncated, "single bundle descriptor is not truncated");

    let (_, descriptor) = rootd::find_payload_by_name(&[], "media-payload")
        .expect("preinstalled exec-bundle payload descriptor resolves before launch");
    testrt::check_eq(descriptor.path, "/payload/media-payload");
    testrt::check_eq(descriptor.entry_name, "payload_media_payload");
    testrt::check(
        payload_store(&[])
            .find_payload("/payload/media-payload")
            .is_none(),
        "payload descriptor preinstall does not install source bytes",
    );

    let loaded = load_payload_by_name(&[], payload_store(&[]), "media-payload")
        .expect("preinstalled payload descriptor still loads from exec bundle");
    testrt::check_eq(loaded.source_bytes(), MEDIA_PAYLOAD_SOURCE);

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/payload/media-payload");
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    reset();
});

arch_test!(exec_loader_discovers_bin_descriptor_from_exec_bundle, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_BIN_CATALOG);

    let loaded = load_bin_program(&[], program_store(&[]), "nosource")
        .expect("exec-bundle-discovered /bin descriptor loads");
    testrt::check_eq(loaded.descriptor.name, "nosource");
    testrt::check_eq(loaded.descriptor.path, "/bin/nosource");
    testrt::check_eq(loaded.descriptor.summary, "provider-discovered /bin program");
    testrt::check_eq(loaded.descriptor.entry_name, "bin_nosource");
    testrt::check_eq(loaded.source_path, "/bin/nosource");
    testrt::check_eq(loaded.source_bytes(), MEDIA_BIN_SOURCE);

    testrt::check(
        program_store(&[]).find_program("/bin/nosource").is_none(),
        "dynamic exec-bundle bin body cache does not install source overlay",
    );

    clear_exec_bundle();
    let by_path = load_bin_program(&[], program_store(&[]), "/bin/nosource")
        .expect("exec-bundle-discovered /bin descriptor reloads from retained body cache");
    testrt::check_eq(by_path.descriptor.entry_name, "bin_nosource");
    testrt::check_eq(by_path.source_bytes(), MEDIA_BIN_SOURCE);

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(records[0].argv0(), "nosource");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].source_path, "/bin/nosource");
    testrt::check_eq(records[0].entry_name, "bin_nosource");
    testrt::check_eq(records[1].argv0(), "/bin/nosource");
    testrt::check_eq(records[1].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[1].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[1].artifact_body_format, ExecArtifactBodyFormat::SourceImage);

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    reset();
});

arch_test!(exec_loader_discovers_payload_descriptor_from_exec_bundle, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_PAYLOAD_DYNAMIC);

    let loaded = load_payload_by_name(&[], payload_store(&[]), "media-payload")
        .expect("exec-bundle-discovered payload descriptor loads");
    testrt::check_eq(loaded.name, "media-payload");
    testrt::check_eq(loaded.path, "/payload/media-payload");
    testrt::check_eq(loaded.summary, "provider-discovered /payload program");
    testrt::check_eq(loaded.entry_name, "payload_media_payload");
    testrt::check_eq(loaded.source_path, "/payload/media-payload");
    testrt::check_eq(loaded.source_bytes(), MEDIA_PAYLOAD_SOURCE);

    testrt::check(
        payload_store(&[])
            .find_payload("/payload/media-payload")
            .is_none(),
        "dynamic exec-bundle payload body cache does not install source overlay",
    );

    clear_exec_bundle();
    let by_path = load_payload_by_name(&[], payload_store(&[]), "/payload/media-payload")
        .expect("exec-bundle-discovered payload descriptor reloads from retained body cache");
    testrt::check_eq(by_path.entry_name, "payload_media_payload");
    testrt::check_eq(by_path.source_bytes(), MEDIA_PAYLOAD_SOURCE);

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(records[0].argv0(), "media-payload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/payload/media-payload");
    testrt::check_eq(records[0].source_path, "/payload/media-payload");
    testrt::check_eq(records[0].entry_name, "payload_media_payload");
    testrt::check_eq(records[1].argv0(), "/payload/media-payload");
    testrt::check_eq(records[1].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[1].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[1].artifact_body_format, ExecArtifactBodyFormat::SourceImage);

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    reset();
});

arch_test!(exec_loader_loads_missing_bin_source_from_source_media_catalog, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_BIN_CATALOG);

    let loaded = load_bin_program(&MISSING_SOURCE_PROGRAMS, program_store(&[]), "nosource")
        .expect("missing source loads from matching source media catalog entry");
    testrt::check_eq(loaded.descriptor.path, "/bin/nosource");
    testrt::check_eq(loaded.source_path, "/bin/nosource");
    testrt::check_eq(loaded.source_bytes(), MEDIA_BIN_SOURCE);

    let installed = program_store(&[])
        .find_program("/bin/nosource")
        .expect("catalog source media installed source overlay");
    testrt::check_eq(installed.bytes, MEDIA_BIN_SOURCE);

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "nosource");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::LoadedFromSourceMedia);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::SourceMediaCatalog);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].source_path, "/bin/nosource");
    let mut expected_artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_artifact_len =
        source_media_encode(&mut expected_artifact, b"bin", b"/bin/nosource", MEDIA_BIN_SOURCE);
    testrt::check_eq(records[0].artifact_format, ExecArtifactFormat::Catalog);
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);
    testrt::check_eq(records[0].artifact_bytes_len, expected_artifact_len);
    testrt::check_eq(records[0].artifact_body_bytes_len, MEDIA_BIN_SOURCE.len());
    testrt::check_eq(records[0].artifact_checksum, source_media_checksum32(MEDIA_BIN_SOURCE));

    reset_installed_sources();
    clear_source_media();
});

arch_test!(exec_loader_discovers_bin_descriptor_from_source_media, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_BIN_DYNAMIC);

    let loaded = load_bin_program(&[], program_store(&[]), "media-bin")
        .expect("media-discovered /bin descriptor loads from source media");
    testrt::check_eq(loaded.descriptor.name, "media-bin");
    testrt::check_eq(loaded.descriptor.path, "/bin/media-bin");
    testrt::check_eq(loaded.descriptor.summary, "provider-discovered /bin program");
    testrt::check_eq(loaded.descriptor.entry_name, "bin_media_bin");
    testrt::check_eq(loaded.source_path, "/bin/media-bin");
    testrt::check_eq(loaded.source_bytes(), MEDIA_BIN_SOURCE);

    let installed = program_store(&[])
        .find_program("/bin/media-bin")
        .expect("dynamic media bin installed source overlay");
    testrt::check_eq(installed.bytes, MEDIA_BIN_SOURCE);

    let by_path = load_bin_program(&[], program_store(&[]), "/bin/media-bin")
        .expect("media-discovered /bin descriptor reloads by absolute path");
    testrt::check_eq(by_path.descriptor.entry_name, "bin_media_bin");

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(records[0].argv0(), "media-bin");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::LoadedFromSourceMedia);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::SourceMediaSingle);
    testrt::check_eq(records[0].path, "/bin/media-bin");
    testrt::check_eq(records[0].source_path, "/bin/media-bin");
    testrt::check_eq(records[0].entry_name, "bin_media_bin");
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);
    testrt::check_eq(records[1].argv0(), "/bin/media-bin");
    testrt::check_eq(records[1].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[1].origin, ExecArtifactOrigin::InstalledOverlay);
    testrt::check_eq(records[1].artifact_body_format, ExecArtifactBodyFormat::SourceImage);

    reset_installed_sources();
    clear_source_media();
    reset();
});

arch_test!(exec_loader_discovers_payload_descriptor_from_source_media, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_PAYLOAD_DYNAMIC);

    let loaded = load_payload_by_name(&[], payload_store(&[]), "media-payload")
        .expect("media-discovered payload descriptor loads from source media");
    testrt::check_eq(loaded.name, "media-payload");
    testrt::check_eq(loaded.path, "/payload/media-payload");
    testrt::check_eq(loaded.summary, "provider-discovered /payload program");
    testrt::check_eq(loaded.entry_name, "payload_media_payload");
    testrt::check_eq(loaded.source_path, "/payload/media-payload");
    testrt::check_eq(loaded.source_bytes(), MEDIA_PAYLOAD_SOURCE);

    let installed = payload_store(&[])
        .find_payload("/payload/media-payload")
        .expect("dynamic media payload installed source overlay");
    testrt::check_eq(installed.bytes, MEDIA_PAYLOAD_SOURCE);

    let by_path = load_payload_by_name(&[], payload_store(&[]), "/payload/media-payload")
        .expect("media-discovered payload descriptor reloads by absolute path");
    testrt::check_eq(by_path.entry_name, "payload_media_payload");

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(records[0].argv0(), "media-payload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::LoadedFromSourceMedia);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::SourceMediaSingle);
    testrt::check_eq(records[0].path, "/payload/media-payload");
    testrt::check_eq(records[0].source_path, "/payload/media-payload");
    testrt::check_eq(records[0].entry_name, "payload_media_payload");
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);
    testrt::check_eq(records[1].argv0(), "/payload/media-payload");
    testrt::check_eq(records[1].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[1].origin, ExecArtifactOrigin::InstalledOverlay);
    testrt::check_eq(records[1].artifact_body_format, ExecArtifactBodyFormat::SourceImage);

    reset_installed_sources();
    clear_source_media();
    reset();
});

arch_test!(exec_loader_rejects_source_media_path_mismatch_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_BIN_WRONG_PATH);

    testrt::check_eq(
        load_bin_program(&MISSING_SOURCE_PROGRAMS, program_store(&[]), "nosource").err(),
        Some(ExecLoadError::SourceNotFound),
    );
    testrt::check(
        program_store(&[]).find_program("/bin/nosource").is_none(),
        "mismatched source media is not installed for requested path",
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "nosource");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::SourceMediaMismatch);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::None);
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].source_path, "/bin/nosource");

    reset_installed_sources();
    clear_source_media();
});

arch_test!(exec_loader_records_payload_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    let payload =
        load_payload_by_name(&TEST_PAYLOADS, payload_store(&TEST_PAYLOAD_SOURCES), "editor-smoke")
            .expect("payload loads by catalog name");
    testrt::check_eq(payload.path, "/payload/editor-smoke");
    testrt::check_eq(payload.image_kind.as_str(), "source-image");
    testrt::check_eq(payload.source_path, "/payload/editor-smoke");
    testrt::check_eq(payload.entry_name, "payload_editor_smoke");

    testrt::check_eq(
        load_payload_by_name(&TEST_PAYLOADS, payload_store(&TEST_PAYLOAD_SOURCES), "").err(),
        Some(ExecLoadError::EmptyArgv0),
    );
    testrt::check_eq(
        load_payload_by_name(&TEST_PAYLOADS, payload_store(&TEST_PAYLOAD_SOURCES), "missing").err(),
        Some(ExecLoadError::NotFound),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 3usize);
    testrt::check_eq(records[0].argv0(), "editor-smoke");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/editor-smoke");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/editor-smoke");
    testrt::check_eq(records[0].entry_name, "payload_editor_smoke");
    testrt::check_eq(records[1].argv0(), "");
    testrt::check_eq(records[1].status, ExecLoadStatus::Error);
    testrt::check_eq(records[1].reason, ExecLoadReason::EmptyArgv0);
    testrt::check_eq(records[1].kind, ExecLoadKind::None);
    testrt::check_eq(records[1].origin, ExecArtifactOrigin::None);
    testrt::check_eq(records[2].argv0(), "missing");
    testrt::check_eq(records[2].status, ExecLoadStatus::Error);
    testrt::check_eq(records[2].reason, ExecLoadReason::NotFound);
    testrt::check_eq(records[2].kind, ExecLoadKind::None);
    testrt::check_eq(records[2].origin, ExecArtifactOrigin::None);
});

arch_test!(exec_loader_uses_installed_payload_source_before_image_source, {
    proc::reset();
    reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            INVALID_PAYLOAD_SOURCE,
        ),
        Ok(()),
    );

    testrt::check_eq(
        load_payload_by_name(&TEST_PAYLOADS, payload_store(&TEST_PAYLOAD_SOURCES), "editor-smoke")
            .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "editor-smoke");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::InstalledOverlay);
    testrt::check_eq(records[0].path, "/payload/editor-smoke");
    testrt::check_eq(records[0].source_path, "/payload/editor-smoke");
    testrt::check_eq(records[0].entry_name, "payload_editor_smoke");
    reset_installed_sources();
});

arch_test!(exec_loader_rejects_invalid_payload_source_image_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_stdout_hex_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_HEX_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_stderr_hex_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_STDERR_HEX_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_child_stdin_hex_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_STDIN_HEX_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_service_ready_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_SERVICE_READY_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_service_hold_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_SERVICE_HOLD_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_vfs_path_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_VFS_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_spawn_bin_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_SPAWN_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_spawn_kill_bin_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_SPAWN_KILL_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_spawn_wait_payload_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_SPAWN_WAIT_PAYLOAD_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_spawn_kill_payload_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_SPAWN_KILL_PAYLOAD_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_sleep_wait_bin_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_SLEEP_WAIT_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_invalid_payload_sleep_bin_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();

    testrt::check_eq(
        load_payload_by_name(
            &INVALID_PAYLOADS,
            payload_store(&INVALID_PAYLOAD_SLEEP_SOURCES),
            "badpayload",
        )
        .err(),
        Some(ExecLoadError::InvalidImage),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "badpayload");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::InvalidImage);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(records[0].path, "/payload/badpayload");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/badpayload");
    testrt::check_eq(records[0].entry_name, "payload_badpayload");
});

arch_test!(exec_loader_rejects_missing_payload_source_at_admission, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_source_media();

    testrt::check_eq(
        load_payload_by_name(&MISSING_SOURCE_PAYLOADS, payload_store(&[]), "missing-source").err(),
        Some(ExecLoadError::SourceNotFound),
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "missing-source");
    testrt::check_eq(records[0].status, ExecLoadStatus::Error);
    testrt::check_eq(records[0].reason, ExecLoadReason::SourceNotFound);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::None);
    testrt::check_eq(records[0].path, "/payload/missing-source");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/missing-source");
    testrt::check_eq(records[0].entry_name, "payload_missing_source");
    reset_installed_sources();
    clear_source_media();
});

arch_test!(exec_loader_loads_missing_payload_source_from_source_media, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_PAYLOAD_MISSING);

    let payload =
        load_payload_by_name(&MISSING_SOURCE_PAYLOADS, payload_store(&[]), "missing-source")
            .expect("missing payload source loads from matching source media");
    testrt::check_eq(payload.path, "/payload/missing-source");
    testrt::check_eq(payload.source_path, "/payload/missing-source");

    let installed = payload_store(&[])
        .find_payload("/payload/missing-source")
        .expect("source media installed payload overlay");
    testrt::check_eq(installed.bytes, MEDIA_PAYLOAD_SOURCE);

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "missing-source");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::LoadedFromSourceMedia);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::SourceMediaSingle);
    testrt::check_eq(records[0].path, "/payload/missing-source");
    testrt::check_eq(records[0].source_path, "/payload/missing-source");
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
});

arch_test!(exec_loader_loads_missing_payload_source_from_exec_bundle, {
    proc::reset();
    reset();
    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
    install_exec_bundle(EXEC_BUNDLE_PAYLOAD_MISSING);

    let payload =
        load_payload_by_name(&MISSING_SOURCE_PAYLOADS, payload_store(&[]), "missing-source")
            .expect("missing payload source loads from matching executable bundle");
    testrt::check_eq(payload.path, "/payload/missing-source");
    testrt::check_eq(payload.source_path, "/payload/missing-source");

    testrt::check(
        payload_store(&[])
            .find_payload("/payload/missing-source")
            .is_none(),
        "exec bundle payload body cache does not install source overlay",
    );

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].argv0(), "missing-source");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Payload);
    testrt::check_eq(records[0].origin, ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(records[0].path, "/payload/missing-source");
    testrt::check_eq(records[0].source_path, "/payload/missing-source");
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::SourceImage);

    reset_installed_sources();
    clear_exec_bundle();
    clear_source_media();
});

arch_test!(exec_pending_program_carries_initial_stdin, {
    proc::reset();
    reset();

    let input = load_bin_program(crate::bin_fixture::programs(), bin_store(), "input")
        .expect("input program loads");
    let handle = spawn_bin_program_with_stdin(input, argv1("input"), b"abc");
    let mut records = [EMPTY_PENDING_EXEC_RECORD; super::MAX_PENDING_EXEC_RECORDS];
    let count = snapshot_pending(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].pid, handle.pid);
    testrt::check_eq(records[0].artifact_body_format, ExecArtifactBodyFormat::LinkedImage);
    testrt::check_eq(records[0].artifact_body_inner_format, ExecArtifactBodyInnerFormat::None);
    testrt::check_eq(records[0].stdin_len, 3usize);

    let pending = take_pending_program(handle.pid).expect("pending program is retained");

    testrt::check_eq(pending.argv().argv0(), Some("input"));
    testrt::check_eq(pending.stdin(), b"abc");
});

arch_test!(exec_pending_program_reclaims_terminal_rows, {
    proc::reset();
    reset();

    let pwd = load_bin_program(crate::bin_fixture::programs(), bin_store(), "pwd")
        .expect("pwd program loads");
    let help = load_bin_program(crate::bin_fixture::programs(), bin_store(), "help")
        .expect("help program loads");

    let terminal = spawn_bin_program(pwd, argv1("pwd"));
    let live = spawn_bin_program(help, argv1("help"));
    let _ = proc::exit_process(terminal.pid, proc::ProcessState::Exited, 0);
    let fresh = spawn_bin_program(help, argv1("help"));

    let mut records = [EMPTY_PENDING_EXEC_RECORD; super::MAX_PENDING_EXEC_RECORDS];
    let count = snapshot_pending(&mut records);
    let mut found_terminal = false;
    let mut found_live = false;
    let mut found_fresh = false;
    let mut index = 0usize;
    while index < count {
        if records[index].pid == terminal.pid {
            found_terminal = true;
        }
        if records[index].pid == live.pid {
            found_live = true;
        }
        if records[index].pid == fresh.pid {
            found_fresh = true;
        }
        index += 1;
    }

    testrt::check(!found_terminal, "terminal pending program row was reclaimed");
    testrt::check(found_live, "live pending program row is preserved");
    testrt::check(found_fresh, "fresh pending program row is retained");
});

arch_test!(exec_pending_program_table_full_fails_without_overwrite, {
    proc::reset();
    reset();

    let program = load_bin_program(crate::bin_fixture::programs(), bin_store(), "help")
        .expect("help program loads");
    let argv = argv1("help");
    let mut handles = [proc::EMPTY_PROCESS_HANDLE; super::MAX_PENDING_EXEC_INVOCATIONS];
    handles[0] = process_handle(proc::ROOTD_PID);
    handles[1] = process_handle(proc::SHELL_PID);

    let mut count = 2usize;
    while count < handles.len() {
        handles[count] = proc::spawn_child_with_program_argv(
            proc::SHELL_PID,
            proc::SHELL_PID,
            program.descriptor.path,
            program.image_kind.as_str(),
            program.descriptor.entry_name,
            &argv,
            program.descriptor.name,
        )
        .expect("live process slot available");
        count += 1;
    }

    let mut state = super::ExecState::new();
    let mut index = 0usize;
    while index < handles.len() {
        testrt::check(
            state.store_pending_program(super::PendingProgramInvocation::new(
                handles[index],
                program,
                argv,
                ProgramEnvBuffer::empty(),
                &[],
            )),
            "initial live pending program row stores",
        );
        index += 1;
    }

    let incoming = proc::ProcessHandle {
        pid: 50_000,
        task_id: 50_000,
        program_path: "/bin/help",
        loader: "linked-bin",
        entry_name: "bin_help",
    };
    let slot = incoming.pid % state.pending_programs.len();
    let before = state.pending_programs[slot]
        .pending
        .expect("protected pending slot exists")
        .handle()
        .pid;

    testrt::check(
        !state.store_pending_program(super::PendingProgramInvocation::new(
            incoming,
            program,
            argv,
            ProgramEnvBuffer::empty(),
            &[],
        )),
        "full live pending program table rejects incoming row",
    );
    let after = state.pending_programs[slot]
        .pending
        .expect("protected pending slot remains")
        .handle()
        .pid;
    testrt::check_eq(after, before);
    testrt::check(
        state.take_pending_program(incoming.pid).is_none(),
        "rejected pending program row was not retained",
    );
});

arch_test!(exec_pending_payload_table_full_fails_without_overwrite, {
    proc::reset();
    reset();
    reset_installed_sources();

    let payload =
        load_payload_by_name(&TEST_PAYLOADS, payload_store(&TEST_PAYLOAD_SOURCES), "editor-smoke")
            .expect("payload loads");
    let argv = argv1("editor-smoke");
    let parent = process_handle(proc::ROOTD_PID);
    let mut handles = [proc::EMPTY_PROCESS_HANDLE; super::MAX_PENDING_EXEC_INVOCATIONS];
    handles[0] = process_handle(proc::ROOTD_PID);
    handles[1] = process_handle(proc::SHELL_PID);

    let mut count = 2usize;
    while count < handles.len() {
        handles[count] = proc::spawn_child_with_program_argv(
            proc::SHELL_PID,
            proc::SHELL_PID,
            payload.path,
            payload.image_kind.as_str(),
            payload.entry_name,
            &argv,
            payload.name,
        )
        .expect("live process slot available");
        count += 1;
    }

    let mut state = super::ExecState::new();
    let mut index = 0usize;
    while index < handles.len() {
        testrt::check(
            state.store_pending_payload(super::PendingPayloadInvocation::new(
                parent,
                handles[index],
                payload,
            )),
            "initial live pending payload row stores",
        );
        index += 1;
    }

    let incoming = proc::ProcessHandle {
        pid: 60_000,
        task_id: 60_000,
        program_path: "/payload/editor-smoke",
        loader: "source-image",
        entry_name: "payload_editor_smoke",
    };
    let slot = incoming.pid % state.pending_payloads.len();
    let before = state.pending_payloads[slot]
        .pending
        .expect("protected payload slot exists")
        .child()
        .pid;

    testrt::check(
        !state
            .store_pending_payload(super::PendingPayloadInvocation::new(parent, incoming, payload)),
        "full live pending payload table rejects incoming row",
    );
    let after = state.pending_payloads[slot]
        .pending
        .expect("protected payload slot remains")
        .child()
        .pid;
    testrt::check_eq(after, before);
    testrt::check(
        state.take_pending_payload(incoming.pid).is_none(),
        "rejected pending payload row was not retained",
    );
});

arch_test!(exec_reset_clears_pending_program_invocations, {
    proc::reset();
    reset();

    let help = load_bin_program(crate::bin_fixture::programs(), bin_store(), "help")
        .expect("help program loads");
    let first = spawn_bin_program(help, argv1("help"));
    testrt::check(
        take_pending_program(first.pid).is_some(),
        "pending program can be taken before reset",
    );

    let second = spawn_bin_program(help, argv1("help"));
    reset();
    testrt::check(
        take_pending_program(second.pid).is_none(),
        "exec reset clears pending program table",
    );
});
