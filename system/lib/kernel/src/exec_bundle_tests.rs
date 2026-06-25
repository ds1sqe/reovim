//! Selftests for the executable bundle provider.

use {
    super::{
        ExecBundleArtifactError, ExecBundleArtifactFormat, ExecBundleCatalogError,
        ExecBundleReadError, find_exec_bundle_catalog_entry, parse_exec_bundle_artifact,
        read_checked_artifact,
    },
    crate::{
        block::{BlockDevice, clear_exec_bundle_device_for_tests, install_exec_bundle_device},
        source_store::{
            MAX_SOURCE_MEDIA_ARTIFACT_BYTES, SourceArtifactNamespace, source_media_checksum32,
        },
    },
    core::sync::atomic::{AtomicUsize, Ordering},
    reovim_testrt::{self as testrt, arch_test},
};

const EXEC_BUNDLE_NONE: usize = 0;
const EXEC_BUNDLE_BIN: usize = 1;
const EXEC_BUNDLE_WRONG_PATH: usize = 2;
const EXEC_BUNDLE_BAD_CHECKSUM: usize = 3;
const EXEC_BUNDLE_CATALOG: usize = 4;
const EXEC_BUNDLE_CATALOG_OFFSET: usize = 256;
static EXEC_BUNDLE_KIND: AtomicUsize = AtomicUsize::new(EXEC_BUNDLE_NONE);

const BUNDLE_BIN_SOURCE: &[u8] = b"reovim-source-v1\nexit-status ok\n";

fn bundle_write(_offset: usize, _bytes: &[u8]) -> bool {
    false
}

fn copy(out: &mut [u8], len: &mut usize, bytes: &[u8]) -> bool {
    if *len + bytes.len() > out.len() {
        return false;
    }
    out[*len..*len + bytes.len()].copy_from_slice(bytes);
    *len += bytes.len();
    true
}

fn copy_usize(out: &mut [u8], len: &mut usize, mut value: usize) -> bool {
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
        if !copy(out, len, &digits[digit_count..digit_count + 1]) {
            return false;
        }
    }
    true
}

fn encode_bundle(out: &mut [u8], path: &[u8], source: &[u8], checksum: usize) -> usize {
    let mut len = 0usize;
    if !copy(out, &mut len, b"reovim-exec-bundle-v1\nnamespace=bin\npath=")
        || !copy(out, &mut len, path)
        || !copy(out, &mut len, b"\nbytes=")
        || !copy_usize(out, &mut len, source.len())
        || !copy(out, &mut len, b"\nchecksum=")
        || !copy_usize(out, &mut len, checksum)
        || !copy(out, &mut len, b"\n")
        || !copy(out, &mut len, source)
    {
        return 0;
    }
    len
}

fn encode_catalog(out: &mut [u8], path: &[u8], source: &[u8], artifact_offset: usize) -> usize {
    let mut artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let artifact_len =
        encode_bundle(&mut artifact, path, source, source_media_checksum32(source) as usize);
    let artifact_checksum = source_media_checksum32(&artifact[..artifact_len]) as usize;

    let mut body = [0u8; 192];
    let mut body_len = 0usize;
    if !copy(&mut body, &mut body_len, b"entry namespace=bin path=")
        || !copy(&mut body, &mut body_len, path)
        || !copy(&mut body, &mut body_len, b" offset=")
        || !copy_usize(&mut body, &mut body_len, artifact_offset)
        || !copy(&mut body, &mut body_len, b" bytes=")
        || !copy_usize(&mut body, &mut body_len, artifact_len)
        || !copy(&mut body, &mut body_len, b" checksum=")
        || !copy_usize(&mut body, &mut body_len, artifact_checksum)
        || !copy(&mut body, &mut body_len, b"\n")
    {
        return 0;
    }

    let checksum = source_media_checksum32(&body[..body_len]) as usize;
    let mut len = 0usize;
    if !copy(out, &mut len, b"reovim-exec-bundle-catalog-v1\nbytes=")
        || !copy_usize(out, &mut len, body_len)
        || !copy(out, &mut len, b"\nchecksum=")
        || !copy_usize(out, &mut len, checksum)
        || !copy(out, &mut len, b"\n")
        || !copy(out, &mut len, &body[..body_len])
    {
        return 0;
    }
    len
}

fn bundle_read(offset: usize, out: &mut [u8]) -> usize {
    let kind = EXEC_BUNDLE_KIND.load(Ordering::Relaxed);
    let checksum = source_media_checksum32(BUNDLE_BIN_SOURCE) as usize;
    match kind {
        EXEC_BUNDLE_BIN if offset == 0 => {
            encode_bundle(out, b"/bin/nosource", BUNDLE_BIN_SOURCE, checksum)
        }
        EXEC_BUNDLE_WRONG_PATH if offset == 0 => {
            encode_bundle(out, b"/bin/other", BUNDLE_BIN_SOURCE, checksum)
        }
        EXEC_BUNDLE_BAD_CHECKSUM if offset == 0 => {
            encode_bundle(out, b"/bin/nosource", BUNDLE_BIN_SOURCE, checksum.wrapping_add(1))
        }
        EXEC_BUNDLE_CATALOG if offset == 0 => {
            encode_catalog(out, b"/bin/nosource", BUNDLE_BIN_SOURCE, EXEC_BUNDLE_CATALOG_OFFSET)
        }
        EXEC_BUNDLE_CATALOG if offset == EXEC_BUNDLE_CATALOG_OFFSET => {
            encode_bundle(out, b"/bin/nosource", BUNDLE_BIN_SOURCE, checksum)
        }
        _ => 0,
    }
}

fn install_bundle(kind: usize) {
    EXEC_BUNDLE_KIND.store(kind, Ordering::Relaxed);
    install_exec_bundle_device(BlockDevice::new(
        "selftest-exec-bundle0",
        MAX_SOURCE_MEDIA_ARTIFACT_BYTES,
        bundle_write,
        bundle_read,
    ));
}

fn clear_bundle() {
    EXEC_BUNDLE_KIND.store(EXEC_BUNDLE_NONE, Ordering::Relaxed);
    clear_exec_bundle_device_for_tests();
}

arch_test!(exec_bundle_reads_checked_artifact, {
    clear_bundle();
    install_bundle(EXEC_BUNDLE_BIN);
    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = read_checked_artifact(SourceArtifactNamespace::Bin, "/bin/nosource", &mut bytes)
        .expect("exec bundle artifact loads");
    testrt::check_eq(read.read.storage, "selftest-exec-bundle0");
    testrt::check_eq(read.format, ExecBundleArtifactFormat::SingleArtifact);
    testrt::check_eq(read.artifact.namespace, SourceArtifactNamespace::Bin);
    testrt::check_eq(read.artifact.path, "/bin/nosource");
    testrt::check_eq(read.artifact.source_bytes, BUNDLE_BIN_SOURCE);
    testrt::check_eq(read.artifact.checksum, source_media_checksum32(BUNDLE_BIN_SOURCE));
    clear_bundle();
});

arch_test!(exec_bundle_reads_catalog_artifact, {
    clear_bundle();
    install_bundle(EXEC_BUNDLE_CATALOG);
    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = read_checked_artifact(SourceArtifactNamespace::Bin, "/bin/nosource", &mut bytes)
        .expect("exec bundle catalog artifact loads");
    testrt::check_eq(read.read.storage, "selftest-exec-bundle0");
    testrt::check_eq(read.read.bytes, read.artifact_bytes_len);
    testrt::check_eq(read.format, ExecBundleArtifactFormat::Catalog);
    testrt::check_eq(read.artifact.namespace, SourceArtifactNamespace::Bin);
    testrt::check_eq(read.artifact.path, "/bin/nosource");
    testrt::check_eq(read.artifact.source_bytes, BUNDLE_BIN_SOURCE);
    clear_bundle();
});

arch_test!(exec_bundle_rejects_path_mismatch, {
    clear_bundle();
    install_bundle(EXEC_BUNDLE_WRONG_PATH);
    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let error = read_checked_artifact(SourceArtifactNamespace::Bin, "/bin/nosource", &mut bytes)
        .expect_err("wrong path is rejected");
    testrt::check_eq(error, ExecBundleReadError::PathMismatch);
    clear_bundle();
});

arch_test!(exec_bundle_catalog_finds_matching_entry, {
    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let len =
        encode_catalog(&mut bytes, b"/bin/nosource", BUNDLE_BIN_SOURCE, EXEC_BUNDLE_CATALOG_OFFSET);
    let entry = find_exec_bundle_catalog_entry(
        &bytes[..len],
        SourceArtifactNamespace::Bin,
        "/bin/nosource",
    )
    .expect("catalog entry is found");
    testrt::check_eq(entry.path, "/bin/nosource");
    testrt::check_eq(entry.offset, EXEC_BUNDLE_CATALOG_OFFSET);
    testrt::check(
        entry.artifact_bytes_len > BUNDLE_BIN_SOURCE.len(),
        "entry records envelope bytes, not source bytes",
    );

    let error =
        find_exec_bundle_catalog_entry(&bytes[..len], SourceArtifactNamespace::Bin, "/bin/other")
            .expect_err("missing catalog entry is rejected");
    testrt::check_eq(error, ExecBundleCatalogError::NotFound);
});

arch_test!(exec_bundle_parser_rejects_bad_checksum, {
    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let len = encode_bundle(&mut bytes, b"/bin/nosource", BUNDLE_BIN_SOURCE, 1);
    let error = parse_exec_bundle_artifact(&bytes[..len]).expect_err("bad checksum is rejected");
    testrt::check_eq(error, ExecBundleArtifactError::ChecksumMismatch);
});
