//! Selftests for executable source-store lookup.

use {
    super::{
        EMPTY_SOURCE_ARTIFACT_RECORD, EMPTY_SOURCE_MEDIA_CATALOG_ENTRY, ExecutableSourceStore,
        MAX_INSTALLED_SOURCE_BYTES, MAX_SOURCE_ARTIFACT_RECORDS, MAX_SOURCE_MEDIA_CATALOG_RECORDS,
        SourceArtifactNamespace, SourceArtifactOrigin, SourceInstallError,
        SourceMediaArtifactError, SourceMediaCatalogError, find_source_media_catalog_entry,
        install_source, parse_source_media_artifact, reset_installed_sources,
        snapshot_source_media_catalog_entries, source_media_checksum32,
    },
    crate::{
        bin_fixture,
        rootd::{PayloadImageKind, PayloadSourceArtifact},
    },
    reovim_testrt::{self as testrt, arch_test},
};

const PAYLOAD_SOURCE: PayloadSourceArtifact = PayloadSourceArtifact {
    path: "/payload/test",
    kind: PayloadImageKind::SourceImage,
    bytes: b"reovim-payload-source-v1\nexit-status ready\n",
};
const INSTALLED_PAYLOAD_SOURCE: &[u8] = b"reovim-payload-source-v1\nexit-status failed\n";
const INSTALLED_BIN_SOURCE: &[u8] = b"reovim-source-v1\nexit-status ok\n";
const VALID_BIN_MEDIA_ARTIFACT: &[u8] = b"reovim-source-media-v1\nnamespace=bin\npath=/bin/proc\nbytes=32\nchecksum=4238279016\nreovim-source-v1\nexit-status ok\n";
const BAD_MEDIA_CHECKSUM_ARTIFACT: &[u8] = b"reovim-source-media-v1\nnamespace=bin\npath=/bin/proc\nbytes=32\nchecksum=1\nreovim-source-v1\nexit-status ok\n";
const BAD_MEDIA_LENGTH_ARTIFACT: &[u8] = b"reovim-source-media-v1\nnamespace=bin\npath=/bin/proc\nbytes=31\nchecksum=4238279016\nreovim-source-v1\nexit-status ok\n";
const VALID_BIN_MEDIA_CATALOG: &[u8] = b"reovim-source-media-catalog-v1\nbytes=75\nchecksum=1265195378\nentry namespace=bin path=/bin/proc offset=128 bytes=113 checksum=870821216\n";
const BAD_MEDIA_CATALOG_CHECKSUM: &[u8] = b"reovim-source-media-catalog-v1\nbytes=75\nchecksum=1\nentry namespace=bin path=/bin/proc offset=128 bytes=113 checksum=870821216\n";

arch_test!(source_store_finds_bin_and_payload_artifacts_by_path, {
    reset_installed_sources();
    let store = ExecutableSourceStore::new(bin_fixture::program_sources(), &[PAYLOAD_SOURCE]);
    testrt::check_eq(store.program_count(), bin_fixture::program_sources().len());
    testrt::check_eq(store.payload_count(), 1usize);

    testrt::check(
        store.find_program("/bin/proc").is_none(),
        "linked image proc has no image source artifact",
    );
    testrt::check(store.find_program("/payload/test").is_none(), "payload not visible as /bin");

    let payload = store
        .find_payload("/payload/test")
        .expect("payload source exists");
    testrt::check_eq(payload.path, "/payload/test");
    testrt::check(
        payload.bytes.starts_with(b"reovim-payload-source-v1\n"),
        "payload source has header",
    );
    testrt::check(store.find_payload("/bin/proc").is_none(), "bin not visible as payload");

    let mut records = [EMPTY_SOURCE_ARTIFACT_RECORD; MAX_SOURCE_ARTIFACT_RECORDS];
    let count = store.snapshot(&mut records);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(records[0].namespace, SourceArtifactNamespace::Payload);
    testrt::check_eq(records[0].path, "/payload/test");
    testrt::check_eq(records[0].loader, "source-image");
    testrt::check(records[0].bytes_len > 0, "payload source has bytes");
    testrt::check_eq(records[0].origin, SourceArtifactOrigin::Image);
    reset_installed_sources();
});

arch_test!(source_store_installed_sources_override_image_sources, {
    reset_installed_sources();
    testrt::check_eq(
        install_source(SourceArtifactNamespace::Payload, "/payload/test", INSTALLED_PAYLOAD_SOURCE),
        Ok(()),
    );
    testrt::check_eq(
        install_source(SourceArtifactNamespace::Bin, "/bin/proc", INSTALLED_BIN_SOURCE),
        Ok(()),
    );

    let store = ExecutableSourceStore::new(bin_fixture::program_sources(), &[PAYLOAD_SOURCE]);
    testrt::check_eq(store.program_count(), 1usize);
    testrt::check_eq(store.payload_count(), 1usize);

    let proc = store
        .find_program("/bin/proc")
        .expect("installed proc source exists");
    testrt::check_eq(proc.path, "/bin/proc");
    testrt::check_eq(proc.bytes, INSTALLED_BIN_SOURCE);

    let payload = store
        .find_payload("/payload/test")
        .expect("installed payload exists");
    testrt::check_eq(payload.path, "/payload/test");
    testrt::check_eq(payload.bytes, INSTALLED_PAYLOAD_SOURCE);

    let mut records = [EMPTY_SOURCE_ARTIFACT_RECORD; MAX_SOURCE_ARTIFACT_RECORDS];
    let count = store.snapshot(&mut records);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(records[0].namespace, SourceArtifactNamespace::Payload);
    testrt::check_eq(records[0].path, "/payload/test");
    testrt::check_eq(records[0].origin, SourceArtifactOrigin::Installed);
    testrt::check_eq(records[1].namespace, SourceArtifactNamespace::Bin);
    testrt::check_eq(records[1].path, "/bin/proc");
    testrt::check_eq(records[1].origin, SourceArtifactOrigin::Installed);

    let mut index = 0usize;
    let mut proc_records = 0usize;
    let mut payload_records = 0usize;
    while index < count {
        if records[index].path == "/bin/proc" {
            proc_records += 1;
        }
        if records[index].path == "/payload/test" {
            payload_records += 1;
        }
        index += 1;
    }
    testrt::check_eq(proc_records, 1usize);
    testrt::check_eq(payload_records, 1usize);
    reset_installed_sources();
});

arch_test!(source_store_rejects_invalid_installed_sources, {
    reset_installed_sources();
    testrt::check_eq(
        install_source(SourceArtifactNamespace::Bin, "", INSTALLED_BIN_SOURCE),
        Err(SourceInstallError::EmptyPath),
    );

    let oversized = [b'x'; MAX_INSTALLED_SOURCE_BYTES + 1];
    testrt::check_eq(
        install_source(SourceArtifactNamespace::Bin, "/bin/too-big", &oversized),
        Err(SourceInstallError::TooLarge),
    );
    reset_installed_sources();
});

arch_test!(source_store_parses_checked_source_media_artifact, {
    let parsed = parse_source_media_artifact(VALID_BIN_MEDIA_ARTIFACT)
        .expect("valid source-media artifact parses");
    testrt::check_eq(parsed.namespace, SourceArtifactNamespace::Bin);
    testrt::check_eq(parsed.path, "/bin/proc");
    testrt::check_eq(parsed.source_bytes, INSTALLED_BIN_SOURCE);
    testrt::check_eq(parsed.checksum, source_media_checksum32(INSTALLED_BIN_SOURCE));
});

arch_test!(source_store_rejects_corrupt_source_media_artifact, {
    testrt::check_eq(
        parse_source_media_artifact(BAD_MEDIA_CHECKSUM_ARTIFACT),
        Err(SourceMediaArtifactError::ChecksumMismatch),
    );
    testrt::check_eq(
        parse_source_media_artifact(BAD_MEDIA_LENGTH_ARTIFACT),
        Err(SourceMediaArtifactError::BodyLengthMismatch),
    );
});

arch_test!(source_store_finds_checked_source_media_catalog_entry, {
    let entry = find_source_media_catalog_entry(
        VALID_BIN_MEDIA_CATALOG,
        SourceArtifactNamespace::Bin,
        "/bin/proc",
    )
    .expect("valid catalog entry found");
    testrt::check_eq(entry.namespace, SourceArtifactNamespace::Bin);
    testrt::check_eq(entry.path, "/bin/proc");
    testrt::check_eq(entry.offset, 128usize);
    testrt::check_eq(entry.artifact_bytes_len, VALID_BIN_MEDIA_ARTIFACT.len());
    testrt::check_eq(entry.checksum, source_media_checksum32(VALID_BIN_MEDIA_ARTIFACT));

    let mut records = [EMPTY_SOURCE_MEDIA_CATALOG_ENTRY; MAX_SOURCE_MEDIA_CATALOG_RECORDS];
    let (count, truncated) =
        snapshot_source_media_catalog_entries(VALID_BIN_MEDIA_CATALOG, &mut records)
            .expect("valid catalog snapshots");
    testrt::check_eq(count, 1usize);
    testrt::check(!truncated, "single-entry source-media catalog is not truncated");
    testrt::check_eq(records[0], entry);
});

arch_test!(source_store_rejects_bad_or_missing_source_media_catalog_entry, {
    testrt::check_eq(
        find_source_media_catalog_entry(
            VALID_BIN_MEDIA_CATALOG,
            SourceArtifactNamespace::Payload,
            "/payload/help",
        ),
        Err(SourceMediaCatalogError::NotFound),
    );
    testrt::check_eq(
        find_source_media_catalog_entry(
            BAD_MEDIA_CATALOG_CHECKSUM,
            SourceArtifactNamespace::Bin,
            "/bin/proc",
        ),
        Err(SourceMediaCatalogError::ChecksumMismatch),
    );
});
