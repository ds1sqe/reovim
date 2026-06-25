//! Selftests for the kernel exec admission service.

use {
    super::{
        EMPTY_EXEC_LOAD_RECORD, ExecLoadError, ExecLoadKind, ExecLoadReason, ExecLoadStatus,
        load_bin_program, load_payload_by_name, reset, snapshot_loads, spawn_bin_program,
        spawn_bin_program_with_stdin, take_pending_program,
    },
    crate::{
        block::{BlockDevice, clear_source_media_device_for_tests, install_source_media_device},
        proc,
        program::{
            ProgramArgvBuffer, ProgramDescriptor, ProgramImage, ProgramImageKind,
            ProgramSourceArtifact,
        },
        rootd::{PayloadDescriptor, PayloadImage, PayloadImageKind, PayloadSourceArtifact},
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
const SOURCE_MEDIA_CATALOG_ARTIFACT_OFFSET: usize = 256;
static SOURCE_MEDIA_KIND: AtomicUsize = AtomicUsize::new(SOURCE_MEDIA_NONE);

const MEDIA_BIN_SOURCE: &[u8] = b"reovim-source-v1\nexit-status ok\n";
const MEDIA_PAYLOAD_SOURCE: &[u8] = b"reovim-payload-source-v1\nexit-status ready\n";

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

fn argv1(arg0: &str) -> ProgramArgvBuffer {
    let mut argv = ProgramArgvBuffer::empty();
    argv.push(arg0).expect("test argv0 fits");
    argv
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
    testrt::check_eq(records[0].path, "/bin/help");
    testrt::check_eq(records[0].loader, help.descriptor.image_kind().as_str());
    testrt::check_eq(records[0].source_path, "/bin/help");
    testrt::check_eq(records[0].entry_name, "bin_help");
    testrt::check_eq(records[2].argv0(), "");
    testrt::check_eq(records[2].status, ExecLoadStatus::Error);
    testrt::check_eq(records[2].reason, ExecLoadReason::EmptyArgv0);
    testrt::check_eq(records[2].kind, ExecLoadKind::None);
    testrt::check_eq(records[3].argv0(), "missing");
    testrt::check_eq(records[3].status, ExecLoadStatus::Error);
    testrt::check_eq(records[3].reason, ExecLoadReason::NotFound);
    testrt::check_eq(records[3].kind, ExecLoadKind::None);
    testrt::check_eq(records[4].argv0(), "/boot/help");
    testrt::check_eq(records[4].status, ExecLoadStatus::Error);
    testrt::check_eq(records[4].reason, ExecLoadReason::NotFound);
    testrt::check_eq(records[4].kind, ExecLoadKind::None);
});

arch_test!(exec_loader_records_source_image_programs, {
    proc::reset();
    reset();

    let pwd = load_bin_program(crate::bin_fixture::programs(), bin_store(), "pwd")
        .expect("pwd loads by basename");
    testrt::check_eq(pwd.descriptor.path, "/bin/pwd");
    testrt::check_eq(pwd.image_kind.as_str(), "source-image");
    testrt::check_eq(pwd.source_path, "/bin/pwd");
    let clear = load_bin_program(crate::bin_fixture::programs(), bin_store(), "clear")
        .expect("clear loads by basename");
    testrt::check_eq(clear.descriptor.path, "/bin/clear");
    testrt::check_eq(clear.image_kind.as_str(), "source-image");
    testrt::check_eq(clear.source_path, "/bin/clear");

    let mut records = [EMPTY_EXEC_LOAD_RECORD; super::MAX_EXEC_LOAD_RECORDS];
    let count = snapshot_loads(&mut records);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(records[0].argv0(), "pwd");
    testrt::check_eq(records[0].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[0].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[0].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[0].path, "/bin/pwd");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/bin/pwd");
    testrt::check_eq(records[0].entry_name, "bin_pwd");
    testrt::check_eq(records[1].argv0(), "clear");
    testrt::check_eq(records[1].status, ExecLoadStatus::Ok);
    testrt::check_eq(records[1].reason, ExecLoadReason::Loaded);
    testrt::check_eq(records[1].kind, ExecLoadKind::Bin);
    testrt::check_eq(records[1].path, "/bin/clear");
    testrt::check_eq(records[1].loader, "source-image");
    testrt::check_eq(records[1].source_path, "/bin/clear");
    testrt::check_eq(records[1].entry_name, "bin_clear");
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
    clear_source_media();
    install_source_media(SOURCE_MEDIA_BIN_NOSOURCE);

    let loaded = load_bin_program(&MISSING_SOURCE_PROGRAMS, program_store(&[]), "nosource")
        .expect("missing source loads from matching source media");
    testrt::check_eq(loaded.descriptor.path, "/bin/nosource");
    testrt::check_eq(loaded.source_path, "/bin/nosource");
    testrt::check_eq(loaded.source_bytes(), MEDIA_BIN_SOURCE);

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
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].source_path, "/bin/nosource");

    reset_installed_sources();
    clear_source_media();
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
    testrt::check_eq(records[0].path, "/bin/nosource");
    testrt::check_eq(records[0].source_path, "/bin/nosource");

    reset_installed_sources();
    clear_source_media();
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
    testrt::check_eq(records[0].path, "/payload/editor-smoke");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check_eq(records[0].source_path, "/payload/editor-smoke");
    testrt::check_eq(records[0].entry_name, "payload_editor_smoke");
    testrt::check_eq(records[1].argv0(), "");
    testrt::check_eq(records[1].status, ExecLoadStatus::Error);
    testrt::check_eq(records[1].reason, ExecLoadReason::EmptyArgv0);
    testrt::check_eq(records[1].kind, ExecLoadKind::None);
    testrt::check_eq(records[2].argv0(), "missing");
    testrt::check_eq(records[2].status, ExecLoadStatus::Error);
    testrt::check_eq(records[2].reason, ExecLoadReason::NotFound);
    testrt::check_eq(records[2].kind, ExecLoadKind::None);
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
    testrt::check_eq(records[0].path, "/payload/missing-source");
    testrt::check_eq(records[0].source_path, "/payload/missing-source");

    reset_installed_sources();
    clear_source_media();
});

arch_test!(exec_pending_program_carries_initial_stdin, {
    proc::reset();
    reset();

    let input = load_bin_program(crate::bin_fixture::programs(), bin_store(), "input")
        .expect("input program loads");
    let handle = spawn_bin_program_with_stdin(input, argv1("input"), b"abc");
    let pending = take_pending_program(handle.pid).expect("pending program is retained");

    testrt::check_eq(pending.argv().argv0(), Some("input"));
    testrt::check_eq(pending.stdin(), b"abc");
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
