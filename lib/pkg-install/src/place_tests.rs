//! Unit tests for the [`place`] module.

use std::fs;

use tempfile::tempdir;

use {
    super::{kind_subdir, place},
    crate::artifact::DiscoveredArtifact,
    reovim_pkg_manifest::PackageKind,
};

fn stage_artifact(src_dir: &std::path::Path, filename: &str, payload: &[u8]) -> DiscoveredArtifact {
    let abs_path = src_dir.join(filename);
    fs::write(&abs_path, payload).unwrap();
    DiscoveredArtifact {
        abs_path,
        filename: filename.to_string(),
    }
}

#[test]
fn place_creates_subdir_and_copies_bytes() {
    let src = tempdir().unwrap();
    let dst = tempdir().unwrap();
    let art = stage_artifact(src.path(), "libreovim_pkg_x.so", b"payload-bytes");

    let pl = place(&art, PackageKind::Module, dst.path()).expect("ok");
    assert!(pl.dst.ends_with("modules/libreovim_pkg_x.so"));
    let copied = fs::read(&pl.dst).unwrap();
    assert_eq!(copied, b"payload-bytes");
}

#[test]
fn place_overwrites_existing_entry() {
    let src = tempdir().unwrap();
    let dst = tempdir().unwrap();
    let art_v1 = stage_artifact(src.path(), "libreovim_pkg_y.so", b"v1");
    place(&art_v1, PackageKind::Driver, dst.path()).expect("first ok");
    fs::write(&art_v1.abs_path, b"v2-bigger").unwrap();
    let pl = place(&art_v1, PackageKind::Driver, dst.path()).expect("second ok");
    assert_eq!(fs::read(&pl.dst).unwrap(), b"v2-bigger");
}

#[test]
fn kind_subdir_matches_loader_convention() {
    assert_eq!(kind_subdir(PackageKind::Driver), "driver");
    assert_eq!(kind_subdir(PackageKind::Module), "modules");
}
