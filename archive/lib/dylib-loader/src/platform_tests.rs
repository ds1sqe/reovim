use super::*;

#[test]
fn library_extension_matches_target_os() {
    let ext = library_extension();
    #[cfg(target_os = "linux")]
    assert_eq!(ext, "so");
    #[cfg(target_os = "macos")]
    assert_eq!(ext, "dylib");
    #[cfg(target_os = "windows")]
    assert_eq!(ext, "dll");
}

#[test]
fn library_filename_follows_os_convention() {
    let name = library_filename("foo");
    #[cfg(target_os = "windows")]
    assert_eq!(name, "foo.dll");
    #[cfg(target_os = "macos")]
    assert_eq!(name, "libfoo.dylib");
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    assert_eq!(name, "libfoo.so");
}

#[test]
fn cdylib_filename_hyphenated_name() {
    let name = cdylib_filename("my-theme");
    #[cfg(target_os = "windows")]
    assert_eq!(name, "reovim_pkg_my_theme.dll");
    #[cfg(target_os = "macos")]
    assert_eq!(name, "libreovim_pkg_my_theme.dylib");
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    assert_eq!(name, "libreovim_pkg_my_theme.so");
}

#[test]
fn cdylib_filename_plain_name() {
    let name = cdylib_filename("alpha");
    #[cfg(target_os = "windows")]
    assert_eq!(name, "reovim_pkg_alpha.dll");
    #[cfg(target_os = "macos")]
    assert_eq!(name, "libreovim_pkg_alpha.dylib");
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    assert_eq!(name, "libreovim_pkg_alpha.so");
}

#[test]
fn pkg_name_from_cdylib_filename_round_trips() {
    for name in ["alpha", "my-theme", "x-y-z"] {
        let composed = cdylib_filename(name);
        assert_eq!(pkg_name_from_cdylib_filename(&composed).as_deref(), Some(name));
    }
}

#[test]
fn pkg_name_from_cdylib_filename_rejects_foreign_filenames() {
    assert!(pkg_name_from_cdylib_filename("not-a-cdylib.txt").is_none());
    assert!(pkg_name_from_cdylib_filename("libfoo.so").is_none());
    #[cfg(not(target_os = "windows"))]
    assert!(pkg_name_from_cdylib_filename("reovim_pkg_alpha.so").is_none());
}
