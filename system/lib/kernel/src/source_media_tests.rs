//! Selftests for checked executable source-media lookup.

use {
    super::{
        SourceMediaArtifactFormat, SourceMediaReadError, SourceMediaSnapshot,
        read_checked_artifact, snapshot_root,
    },
    crate::{
        block::{BlockDevice, clear_source_media_device_for_tests, install_source_media_device},
        source_store::{
            EMPTY_SOURCE_MEDIA_CATALOG_ENTRY, MAX_SOURCE_MEDIA_ARTIFACT_BYTES,
            MAX_SOURCE_MEDIA_CATALOG_RECORDS, SourceArtifactNamespace, source_media_checksum32,
        },
    },
    core::sync::atomic::{AtomicUsize, Ordering},
    reovim_testrt::{self as testrt, arch_test},
};

const SOURCE_MEDIA_NONE: usize = 0;
const SOURCE_MEDIA_SINGLE: usize = 1;
const SOURCE_MEDIA_CATALOG: usize = 2;
const SOURCE_MEDIA_WRONG_PATH: usize = 3;
const SOURCE_MEDIA_ARTIFACT_OFFSET: usize = 256;
const SOURCE_MEDIA_CAPACITY: usize = 1024;
const MEDIA_BIN_SOURCE: &[u8] = b"reovim-source-v1\nexit-status ok\n";

static SOURCE_MEDIA_KIND: AtomicUsize = AtomicUsize::new(SOURCE_MEDIA_NONE);
static LAST_READ_OFFSET: AtomicUsize = AtomicUsize::new(usize::MAX);

fn source_media_write(_offset: usize, _bytes: &[u8]) -> bool {
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

fn encode_artifact(out: &mut [u8], namespace: &[u8], path: &[u8], source: &[u8]) -> usize {
    let mut len = 0usize;
    let checksum = source_media_checksum32(source) as usize;
    if !copy(out, &mut len, b"reovim-source-media-v1\nnamespace=")
        || !copy(out, &mut len, namespace)
        || !copy(out, &mut len, b"\npath=")
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

fn encode_catalog(out: &mut [u8], namespace: &[u8], path: &[u8]) -> usize {
    let mut artifact = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let artifact_len = encode_artifact(&mut artifact, namespace, path, MEDIA_BIN_SOURCE);
    let artifact_checksum = source_media_checksum32(&artifact[..artifact_len]) as usize;

    let mut body = [0u8; 192];
    let mut body_len = 0usize;
    if !copy(&mut body, &mut body_len, b"entry namespace=")
        || !copy(&mut body, &mut body_len, namespace)
        || !copy(&mut body, &mut body_len, b" path=")
        || !copy(&mut body, &mut body_len, path)
        || !copy(&mut body, &mut body_len, b" offset=")
        || !copy_usize(&mut body, &mut body_len, SOURCE_MEDIA_ARTIFACT_OFFSET)
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
    if !copy(out, &mut len, b"reovim-source-media-catalog-v1\nbytes=")
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

fn source_media_read(offset: usize, out: &mut [u8]) -> usize {
    LAST_READ_OFFSET.store(offset, Ordering::Relaxed);
    match SOURCE_MEDIA_KIND.load(Ordering::Relaxed) {
        SOURCE_MEDIA_SINGLE if offset == 0 => {
            encode_artifact(out, b"bin", b"/bin/media-bin", MEDIA_BIN_SOURCE)
        }
        SOURCE_MEDIA_CATALOG if offset == 0 => encode_catalog(out, b"bin", b"/bin/media-bin"),
        SOURCE_MEDIA_CATALOG if offset == SOURCE_MEDIA_ARTIFACT_OFFSET => {
            encode_artifact(out, b"bin", b"/bin/media-bin", MEDIA_BIN_SOURCE)
        }
        SOURCE_MEDIA_WRONG_PATH if offset == 0 => {
            encode_artifact(out, b"bin", b"/bin/other", MEDIA_BIN_SOURCE)
        }
        _ => 0,
    }
}

fn install_source_media(kind: usize) {
    SOURCE_MEDIA_KIND.store(kind, Ordering::Relaxed);
    LAST_READ_OFFSET.store(usize::MAX, Ordering::Relaxed);
    install_source_media_device(BlockDevice::new(
        "source-media-test0",
        SOURCE_MEDIA_CAPACITY,
        source_media_write,
        source_media_read,
    ));
}

fn clear_source_media() {
    SOURCE_MEDIA_KIND.store(SOURCE_MEDIA_NONE, Ordering::Relaxed);
    LAST_READ_OFFSET.store(usize::MAX, Ordering::Relaxed);
    clear_source_media_device_for_tests();
}

arch_test!(source_media_reads_checked_artifact_from_catalog, {
    clear_source_media();
    install_source_media(SOURCE_MEDIA_CATALOG);

    let mut artifact_bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read =
        read_checked_artifact(SourceArtifactNamespace::Bin, "/bin/media-bin", &mut artifact_bytes)
            .expect("catalog source-media artifact reads");

    testrt::check_eq(read.read.storage, "source-media-test0");
    testrt::check_eq(read.read.capacity_bytes, SOURCE_MEDIA_CAPACITY);
    testrt::check_eq(read.format, SourceMediaArtifactFormat::Catalog);
    testrt::check(read.artifact_bytes_len > MEDIA_BIN_SOURCE.len(), "envelope length recorded");
    testrt::check_eq(read.artifact.namespace, SourceArtifactNamespace::Bin);
    testrt::check_eq(read.artifact.path, "/bin/media-bin");
    testrt::check_eq(read.artifact.source_bytes, MEDIA_BIN_SOURCE);
    testrt::check_eq(LAST_READ_OFFSET.load(Ordering::Relaxed), SOURCE_MEDIA_ARTIFACT_OFFSET);

    clear_source_media();
});

arch_test!(source_media_snapshots_catalog_and_single_artifact, {
    clear_source_media();
    install_source_media(SOURCE_MEDIA_CATALOG);

    let mut root = [0u8; crate::source_store::MAX_SOURCE_MEDIA_CATALOG_BYTES];
    let mut entries = [EMPTY_SOURCE_MEDIA_CATALOG_ENTRY; MAX_SOURCE_MEDIA_CATALOG_RECORDS];
    match snapshot_root(&mut root, &mut entries) {
        SourceMediaSnapshot::Catalog {
            read,
            count,
            truncated,
        } => {
            testrt::check_eq(read.storage, "source-media-test0");
            testrt::check_eq(count, 1usize);
            testrt::check_eq(truncated, false);
            testrt::check_eq(entries[0].namespace, SourceArtifactNamespace::Bin);
            testrt::check_eq(entries[0].path, "/bin/media-bin");
            testrt::check_eq(entries[0].offset, SOURCE_MEDIA_ARTIFACT_OFFSET);
        }
        _ => testrt::check(false, "catalog snapshot expected"),
    }

    install_source_media(SOURCE_MEDIA_SINGLE);
    let mut root_single = [0u8; crate::source_store::MAX_SOURCE_MEDIA_CATALOG_BYTES];
    let mut entries_single = [EMPTY_SOURCE_MEDIA_CATALOG_ENTRY; MAX_SOURCE_MEDIA_CATALOG_RECORDS];
    match snapshot_root(&mut root_single, &mut entries_single) {
        SourceMediaSnapshot::Artifact { artifact, .. } => {
            testrt::check_eq(artifact.path, "/bin/media-bin");
            testrt::check_eq(artifact.source_bytes, MEDIA_BIN_SOURCE);
        }
        _ => testrt::check(false, "single artifact snapshot expected"),
    }

    clear_source_media();
});

arch_test!(source_media_rejects_checked_artifact_path_mismatch, {
    clear_source_media();
    install_source_media(SOURCE_MEDIA_WRONG_PATH);

    let mut artifact_bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let error =
        read_checked_artifact(SourceArtifactNamespace::Bin, "/bin/media-bin", &mut artifact_bytes)
            .expect_err("path mismatch rejected");
    testrt::check_eq(error, SourceMediaReadError::PathMismatch);

    clear_source_media();
});
