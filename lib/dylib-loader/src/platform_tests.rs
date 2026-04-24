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
