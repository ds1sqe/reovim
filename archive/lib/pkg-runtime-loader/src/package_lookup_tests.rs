//! Tests for [`super::package_name_for_path`].

use std::path::{Path, PathBuf};

use reovim_dylib_loader::cdylib_filename;

use super::package_name_for_path;

#[test]
fn convention_filename_round_trips_to_package_name() {
    let filename = cdylib_filename("my-theme");
    let path = PathBuf::from("/lib/reovim/modules").join(&filename);
    assert_eq!(package_name_for_path(&path).as_deref(), Some("my-theme"));
}

#[test]
fn non_convention_filename_returns_none() {
    let path = Path::new("/usr/lib/libfoo.so");
    assert_eq!(package_name_for_path(path), None);
}

#[test]
fn path_without_filename_returns_none() {
    assert_eq!(package_name_for_path(Path::new("/")), None);
}

#[cfg(unix)]
#[test]
fn non_utf8_filename_returns_none() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let mut bytes = b"libreovim_pkg_".to_vec();
    bytes.push(0xff); // invalid UTF-8 byte inside the snake-name segment
    bytes.extend_from_slice(b".so");
    let path = PathBuf::from(OsString::from_vec(bytes));
    assert_eq!(package_name_for_path(&path), None);
}
