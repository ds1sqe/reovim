use super::*;

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn server_bin_ends_with_reovim_server() {
    let path = which_reovim_server();
    let name = path
        .file_name()
        .expect("has filename")
        .to_string_lossy()
        .to_string();
    #[cfg(windows)]
    assert!(name == "reovim-server.exe", "unexpected filename: {name}");
    #[cfg(not(windows))]
    assert!(name == "reovim-server", "unexpected filename: {name}");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn tui_bin_ends_with_reovim_tui() {
    let path = which_reovim_tui();
    let name = path
        .file_name()
        .expect("has filename")
        .to_string_lossy()
        .to_string();
    #[cfg(windows)]
    assert!(name == "reovim-tui.exe", "unexpected filename: {name}");
    #[cfg(not(windows))]
    assert!(name == "reovim-tui", "unexpected filename: {name}");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn bin_filename_linux_and_macos_has_no_exe_suffix() {
    #[cfg(not(windows))]
    assert_eq!(bin_filename("reovim-server"), "reovim-server");
    #[cfg(windows)]
    assert_eq!(bin_filename("reovim-server"), "reovim-server.exe");
}
