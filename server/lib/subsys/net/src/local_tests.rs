use super::*;

#[test]
#[cfg(unix)]
fn test_local_addr_for_instance_unix() {
    let addr = LocalAddr::for_instance("myproject");
    match addr {
        LocalAddr::UnixSocket(path) => {
            assert!(path.to_string_lossy().contains("myproject.sock"));
            assert!(path.to_string_lossy().contains("reovim"));
        }
    }
}

#[test]
#[cfg(unix)]
fn test_socket_dir_exists_or_can_be_created() {
    let dir = LocalAddr::socket_dir();
    // Should be a valid path (though directory might not exist yet)
    assert!(!dir.as_os_str().is_empty());
}

#[test]
#[cfg(windows)]
fn test_local_addr_for_instance_windows() {
    let addr = LocalAddr::for_instance("myproject");
    match addr {
        LocalAddr::NamedPipe(name) => {
            assert_eq!(name, r"\\.\pipe\reovim-myproject");
        }
    }
}

#[test]
fn test_local_addr_display() {
    let addr = LocalAddr::for_instance("test");
    let s = addr.to_string();
    assert!(s.contains("test"));
}

#[test]
#[cfg(unix)]
fn test_socket_path_accessor() {
    let addr = LocalAddr::for_instance("accessor-test");
    let path = addr.socket_path();
    assert!(path.to_string_lossy().contains("accessor-test.sock"));
}

#[test]
#[cfg(unix)]
fn test_socket_path_for_instance() {
    let path = LocalAddr::socket_path_for_instance("my-inst");
    assert!(path.to_string_lossy().contains("my-inst.sock"));
    assert!(path.to_string_lossy().contains("reovim"));
}

#[test]
fn test_local_addr_equality() {
    let a = LocalAddr::for_instance("same");
    let b = LocalAddr::for_instance("same");
    assert_eq!(a, b);

    let c = LocalAddr::for_instance("different");
    assert_ne!(a, c);
}

#[test]
fn test_local_addr_clone() {
    let a = LocalAddr::for_instance("clone-test");
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn test_local_addr_debug() {
    let addr = LocalAddr::for_instance("debug-test");
    let debug_str = format!("{addr:?}");
    assert!(debug_str.contains("debug-test"));
}

#[test]
fn test_local_addr_display_format() {
    let addr = LocalAddr::for_instance("display-test");
    let display = format!("{addr}");
    assert!(display.contains("display-test"));
}

#[test]
#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_socket_dir_contains_reovim() {
    let dir = LocalAddr::socket_dir();
    let dir_str = dir.to_string_lossy();
    assert!(dir_str.contains("reovim"), "Socket dir should contain 'reovim': {dir_str}");
}

#[test]
#[cfg(unix)]
fn test_multiple_instances_different_paths() {
    let a = LocalAddr::for_instance("inst-a");
    let b = LocalAddr::for_instance("inst-b");
    assert_ne!(a, b);

    let path_a = a.socket_path();
    let path_b = b.socket_path();
    assert_ne!(path_a, path_b);
}
